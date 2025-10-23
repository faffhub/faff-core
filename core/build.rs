use vergen_gix::{Emitter, GixBuilder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let gitcl = GixBuilder::all_git()?;

    // Emit the build instructions
    Emitter::default().add_instructions(&gitcl)?.emit()?;
    Ok(())
}
