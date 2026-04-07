fn main() {
    uniffi::generate_scaffolding("./src/faff_core.udl").expect("Failed to generate UniFFI scaffolding");
}
