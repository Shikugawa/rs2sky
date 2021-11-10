fn main() -> Result<(), Box<dyn std::error::Error>> {
  tonic_build::configure().compile(
      &["../skywalking-data-collect-protocol/language-agent/Tracing.proto"],
      &["../skywalking-data-collect-protocol"],
  )?;
  Ok(())
}
