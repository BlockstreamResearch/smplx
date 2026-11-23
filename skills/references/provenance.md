# Sources and compatibility

This skill is maintained in the repository's root `skills/` directory and bundled into the CLI at compile time. `simplex skill path` exposes the entire bundle, including references and assets. The content hash in its cache path changes when any packaged file changes.

Target: Simplex 0.2.0 with SimplicityHL 0.8.0, as declared in the workspace manifests. The source checker and execution fixture use this toolchain. Earlier guidance was developed against Simplex 0.0.12 / SimplicityHL 0.7.2; the original sources below preserve that provenance.

Do not fetch these URLs when the task prohibits external research. They document provenance rather than a runtime dependency.

- https://docs.simplicity-lang.org/ and https://docs.simplicity-lang.org/llms-full.txt
- https://docs.simplicity-lang.org/simplicityhl-reference/match_expression/
- https://docs.simplicity-lang.org/simplicityhl-reference/builtin/
- https://docs.simplicity-lang.org/documentation/timelocks/
- https://docs.simplicity-lang.org/documentation/simplex/
- https://docs.simplicity-lang.org/documentation/witness-format/
- https://github.com/BlockstreamResearch/smplx at `5472e9149d60f4b618e827c061c395aa7215c137`
- Simplex binary release https://github.com/BlockstreamResearch/smplx/releases/tag/v0.0.12

The CLI source defines `Simplex.toml`. Source discovery now warns about unparseable files; still verify intended entrypoint coverage. For SDK examples, use the generated bindings and `examples/basic/` from the matching Simplex version.
