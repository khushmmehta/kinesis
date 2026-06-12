use kinesis::run;

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    run()?;

    Ok(())
}
