use anyhow::{anyhow, Result};
use ark_bn254::{Bn254, G1Projective};
use ark_ec::{pairing::Pairing, CurveGroup, VariableBaseMSM};
use rand::rngs::OsRng;
use substrate_bn::{pairing_batch, AffineG1, Fr, G1, G2};

use crate::{
    constants::{ERR_INVALID_NUMBER_OF_DIGESTS, ERR_PAIRING_CHECK_FAILED, GAMMA},
    groth16::{
        convert_fr_sub_to_ark, convert_g1_ark_to_sub, convert_g1_sub_to_ark, convert_g2_sub_to_ark,
    },
    transcript::Transcript,
};

use super::{converter::g1_to_bytes, element::PlonkFr};
use num_traits::Zero;

pub(crate) type Digest = AffineG1;

#[derive(Clone, Copy, Debug)]
#[allow(dead_code)]
pub(crate) struct E2 {
    pub(crate) a0: Fr,
    pub(crate) a1: Fr,
}

#[derive(Clone, Copy, Debug)]
#[allow(dead_code)]
pub(crate) struct LineEvaluationAff {
    pub(crate) r0: E2,
    pub(crate) r1: E2,
}

#[derive(Clone, Copy, Debug)]
#[allow(dead_code)]
pub(crate) struct KZGVerifyingKey {
    pub(crate) g2: [G2; 2], // [G₂, [α]G₂]
    pub(crate) g1: G1,
    // Precomputed pairing lines corresponding to G₂, [α]G₂
    pub(crate) lines: [[[LineEvaluationAff; 66]; 2]; 2],
}

#[derive(Clone, Debug)]
pub(crate) struct BatchOpeningProof {
    pub(crate) h: AffineG1,
    pub(crate) claimed_values: Vec<Fr>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct OpeningProof {
    pub(crate) h: AffineG1,
    pub(crate) claimed_value: Fr,
}

fn derive_gamma(
    point: &Fr,
    digests: Vec<Digest>,
    claimed_values: Vec<Fr>,
    data_transcript: Option<Vec<u8>>,
) -> Result<Fr> {
    let mut transcript = Transcript::new(Some([GAMMA.to_string()].to_vec()))?;
    transcript.bind(GAMMA, &point.into_u256().to_bytes_be())?;

    for digest in digests.iter() {
        transcript.bind(GAMMA, &g1_to_bytes(digest)?)?;
    }

    for claimed_value in claimed_values.iter() {
        transcript.bind(GAMMA, &claimed_value.into_u256().to_bytes_be())?;
    }

    if let Some(data_transcript) = data_transcript {
        transcript.bind(GAMMA, &data_transcript)?;
    }

    let gamma_byte = transcript.compute_challenge(GAMMA)?;
    println!("gamma_byte: {:?}", gamma_byte);
    let x = PlonkFr::set_bytes(&gamma_byte.as_slice())?.into_fr()?;

    Ok(x)
}

fn fold(di: Vec<Digest>, fai: Vec<Fr>, ci: Vec<Fr>) -> Result<(G1, Fr)> {
    let nb_digests = di.len();

    let mut folded_evaluations = Fr::zero();

    for i in 0..nb_digests {
        folded_evaluations += fai[i] * ci[i];
    }

    let msm = G1Projective::msm(
        &di.iter()
            .map(|p| convert_g1_sub_to_ark(*p))
            .collect::<Vec<_>>(),
        &ci.iter()
            .map(|s| convert_fr_sub_to_ark(*s))
            .collect::<Vec<_>>(),
    )
    .map_err(|e| anyhow!(e))?
    .into_affine();
    let folded_digests = convert_g1_ark_to_sub(msm);
    // let folded_digests = G1::msm(&di.iter().map(|&p| p.into()).collect::<Vec<_>>(), &ci);

    Ok((folded_digests.into(), folded_evaluations))
}

pub(crate) fn fold_proof(
    digests: Vec<Digest>,
    batch_opening_proof: &BatchOpeningProof,
    point: &Fr,
    data_transcript: Option<Vec<u8>>,
) -> Result<(OpeningProof, G1)> {
    let nb_digests = digests.len();

    if nb_digests != batch_opening_proof.claimed_values.len() {
        return Err(anyhow!(ERR_INVALID_NUMBER_OF_DIGESTS));
    }
    let gamma = derive_gamma(
        point,
        digests.clone(),
        batch_opening_proof.claimed_values.clone(),
        data_transcript,
    )?;
    println!("gamma as ark-bn: {:?}", convert_fr_sub_to_ark(gamma));

    let mut gammai = vec![Fr::zero(); nb_digests];
    gammai[0] = Fr::one();
    if nb_digests > 1 {
        gammai[1] = gamma;
    }
    for i in 2..nb_digests {
        gammai[i] = gammai[i - 1] * gamma;
    }
    gammai.iter().enumerate().for_each(|(i, gamma)| {
        println!("gamma[{i}] as ark-bn: {:?}", convert_fr_sub_to_ark(*gamma));
    });

    let (folded_digests, folded_evaluations) =
        fold(digests, batch_opening_proof.claimed_values.clone(), gammai)?;

    println!(
        "folded_evaluations as ark-bn: {:?}",
        convert_fr_sub_to_ark(folded_evaluations)
    );
    println!(
        "folded_digests as ark-bn: {:?}",
        convert_g1_sub_to_ark(folded_digests.into())
    );

    let open_proof = OpeningProof {
        h: batch_opening_proof.h,
        claimed_value: folded_evaluations,
    };

    Ok((open_proof, folded_digests))
}

// fn verify(
//     commitment: &Digest,
//     proof: &OpeningProof,
//     point: &Fr,
//     vk: &PlonkVerifyingKey,
// ) -> Result<bool, &'static str> {
//     let mut total_g1 = G1::zero();
//     let point_neg = -(*point);
//     let cm_int = proof.claimed_value.into_repr();
//     let point_int = point_neg.into_repr();

//     // Perform joint scalar multiplication
//     let scalars = vec![cm_int, point_int];
//     let bases = vec![vk.g1.into_projective(), proof.h.into_projective()];
//     total_g1 = G1Projective::msm_unchecked(&bases, &scalars);

//     // [f(a) - a*H(α)]G1 + [-f(α)]G1 = [f(a) - f(α) - a*H(α)]G1
//     let commitment_jac = commitment.0.into_projective();
//     total_g1 -= commitment_jac;

//     // Convert total_g1 to affine
//     let total_g1_aff = total_g1.into_affine();

//     // Perform the pairing check
//     let check = Bn254::product_of_pairings(&[
//         (total_g1_aff.prepare(), vk.lines[0].clone()),
//         (proof.h.prepare(), vk.lines[1].clone()),
//     ]);

//     // Check if the result is 1 (pairing check passed)
//     if check.is_one() {
//         Ok(())
//     } else {
//         Err("Verification failed".into())
//     }
// }

pub(crate) fn batch_verify_multi_points(
    digests: Vec<Digest>,
    proofs: Vec<OpeningProof>,
    points: Vec<Fr>,
    vk: &KZGVerifyingKey,
) -> Result<()> {
    println!("digests:");
    for (i, digest) in digests.iter().enumerate() {
        println!("  digest[{}]: {:?}", i, convert_g1_sub_to_ark(*digest));
    }
    println!("proofs:");
    println!("proofs:");
    for (i, proof) in proofs.iter().enumerate() {
        println!("  proof[{}]:", i);
        println!("    h: {:?}", convert_g1_sub_to_ark(proof.h));
        println!(
            "    claimed_value: {:?}",
            convert_fr_sub_to_ark(proof.claimed_value)
        );
    }
    println!("points:");
    for (i, point) in points.iter().enumerate() {
        println!("  point[{}]: {:?}", i, convert_fr_sub_to_ark(*point));
    }
    println!("vk.g1: {:?}", convert_g1_sub_to_ark(vk.g1.into()));
    println!("vk.g2[0]: {:?}", convert_g2_sub_to_ark(vk.g2[0].into()));
    println!("vk.g2[1]: {:?}", convert_g2_sub_to_ark(vk.g2[1].into()));
    println!("vk.lines:");
    for i in 0..2 {
        for j in 0..2 {
            println!("  vk.lines[{i}][{j}]: {:?}", vk.lines[i][j]);
        }
    }

    let nb_digests = digests.len();
    let nb_proofs = proofs.len();
    let nb_points = points.len();

    if nb_digests != nb_proofs {
        return Err(anyhow!(ERR_INVALID_NUMBER_OF_DIGESTS));
    }

    if nb_digests != nb_points {
        return Err(anyhow!(ERR_INVALID_NUMBER_OF_DIGESTS));
    }

    if nb_digests == 1 {
        todo!();
    }

    let mut rng = OsRng;
    let mut random_numbers = Vec::with_capacity(nb_digests);
    random_numbers.push(Fr::one());
    for _ in 1..nb_digests {
        // random_numbers.push(Fr::random(&mut rng));
        random_numbers.push(Fr::one());
    }

    let mut quotients = Vec::with_capacity(nb_proofs);
    for i in 0..random_numbers.len() {
        quotients.push(proofs[i].h);
    }

    // let mut folded_quotients = G1::msm(
    //     &quotients.iter().map(|&p| p.into()).collect::<Vec<_>>(),
    //     &random_numbers,
    // );
    println!("quotients:");
    for (i, quotient) in quotients.iter().enumerate() {
        println!("  quotient[{}]: {:?}", i, convert_g1_sub_to_ark(*quotient));
    }
    let msm = G1Projective::msm(
        &quotients
            .iter()
            .map(|p| convert_g1_sub_to_ark(*p))
            .collect::<Vec<_>>(),
        &random_numbers
            .iter()
            .map(|s| convert_fr_sub_to_ark(*s))
            .collect::<Vec<_>>(),
    )
    .map_err(|e| anyhow!(e))?
    .into_affine();
    println!("folded_quotients: {:?}", msm);
    let mut folded_quotients: G1 = convert_g1_ark_to_sub(msm).into();

    let mut evals = Vec::with_capacity(nb_digests);
    for i in 0..nb_digests {
        evals.push(proofs[i].claimed_value);
    }

    let (mut folded_digests, folded_evals) = fold(digests, evals, random_numbers.clone())?;
    println!("folded_evals: {:?}", convert_fr_sub_to_ark(folded_evals));
    println!(
        "folded_digests: {:?}",
        convert_g1_sub_to_ark(folded_digests.into())
    );
    let folded_evals_commit = vk.g1 * folded_evals;
    folded_digests = folded_digests - folded_evals_commit;

    for i in 0..random_numbers.len() {
        random_numbers[i] = random_numbers[i] * points[i];
    }

    // let folded_points_quotients = G1::msm(
    //     &quotients.iter().map(|&p| p.into()).collect::<Vec<_>>(),
    //     &random_numbers,
    // );
    let msm = G1Projective::msm(
        &quotients
            .iter()
            .map(|p| convert_g1_sub_to_ark(*p))
            .collect::<Vec<_>>(),
        &random_numbers
            .iter()
            .map(|s| convert_fr_sub_to_ark(*s))
            .collect::<Vec<_>>(),
    )
    .map_err(|e| anyhow!(e))?
    .into_affine();
    println!("folded_points_quotients: {:?}", msm);
    let folded_points_quotients: G1 = convert_g1_ark_to_sub(msm).into();

    folded_digests = folded_digests + folded_points_quotients;
    folded_quotients = -folded_quotients;

    // Pairing check
    // let pairing_result = pairing_batch(&[(folded_digests, vk.g2[0]), (folded_quotients, vk.g2[1])]);
    let pairing_result = Bn254::multi_pairing(
        [
            convert_g1_sub_to_ark(folded_digests.into()),
            convert_g1_sub_to_ark(folded_quotients.into()),
        ],
        [
            convert_g2_sub_to_ark(vk.g2[0].into()),
            convert_g2_sub_to_ark(vk.g2[1].into()),
        ],
    );

    if !pairing_result.is_zero() {
        return Err(anyhow!(ERR_PAIRING_CHECK_FAILED));
    }

    Ok(())
}
