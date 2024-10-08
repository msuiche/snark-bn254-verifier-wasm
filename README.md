# snark-bn254-verifier

The `snark-bn254-verifier` crate is used for verifying Groth16 and PlonK proofs on the `Bn254` curve, ensuring compatibility with proofs generated by the `gnark` library. One can save proofs and verification keys from `gnark` and subsequently load them to this library for verifying the proofs with ease.

## Getting started
### Step 1 (Optional)
Retrieving the proofs and verification keys from gnark.

Retrieve the verification keys from the `vk` folder.
```
cd vk/circuits/
cargo run
```

### Step 2.
Compile the wasm module and run the server:
```
cd verifier/
wasm-pack build --target web && python -m http.server 8000
```

You can then pass the .bin files from the examples/binaries/ folder to the wasm module to verify the proofs.

#### Run the server

If you want to use the wasm module in the browser, you can use the `verifier` folder. This folder contains a simple html file that allows you to verify proofs using the wasm module.

To run the server:
```
cd verifier/
python -m http.server 8000
```

## Features

- Verification of Groth16 and PlonK proofs generated using `gnark` or `sp1` on the `Bn254` curve.
- Easy integration into Rust projects.

### SP1 from WASM

To use SP1 from WebAssembly (WASM), you need to add the following dependency to your `Cargo.toml` file:

sp1-sdk = { version = "2.0.0", default-features = false }
