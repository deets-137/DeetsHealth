// The dev inspector (docs/ops/DEBUGGING.md): `npm run inspect -- --help`.
// Built only with the `dev` feature; a release never contains it.

fn main() {
    std::process::exit(deetshealth_lib::dev_tools::inspect(std::env::args().skip(1).collect()));
}
