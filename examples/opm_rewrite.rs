//! Reads an `.opm` file and writes it back in OP2MissionEditor's layout: `opm_rewrite <in.opm> <out.opm>`.
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let [_, input, output] = &args[..] else {
        eprintln!("usage: opm_rewrite <in.opm> <out.opm>");
        std::process::exit(2);
    };
    let mission = op2_formats::opm::Mission::open(input).unwrap_or_else(|e| {
        eprintln!("{input}: {e}");
        std::process::exit(1);
    });
    mission.save(output).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(1);
    });
}
