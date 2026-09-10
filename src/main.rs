//! **optimizer** — o binário do schematize Optimizer.
//!
//! **Onde:** ponto de entrada. Instalado sozinho, ou pelo schematize (ADR-0011).

mod cli;

use clap::Parser;
use cli::args::{Cli, Cmd};

/// **O quê:** devolve o `SIGPIPE` ao padrão do Unix.
///
/// **Por quê:** o runtime do Rust o põe em `SIG_IGN`, e aí escrever num pipe fechado vira
/// `EPIPE` — que o `println!` transforma em **panic**. `optimizer diag | head` é uso
/// corriqueiro (§37.48). Mesma correção que o Deployer levou.
#[cfg(unix)]
fn restaurar_sigpipe() {
    // SAFETY: chamada única, antes de qualquer thread, restaurando o handler padrão do SO.
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }
}
#[cfg(not(unix))]
fn restaurar_sigpipe() {}

fn main() {
    restaurar_sigpipe();
    let cli = Cli::parse();
    let r = match cli.cmd {
        Cmd::Diag { wait, json } => cli::diag::diag_cmd(wait, json),
        Cmd::Limits { apply, revert, json } => cli::caixas::caixas_cmd(apply, revert, json),
        Cmd::Services { json } => cli::servicos::servicos_cmd(json),
        Cmd::Desktop { install, remove } => cli::diag::desktop_cmd(install, remove),
    };
    if let Err(e) = r {
        eprintln!("erro: {e}");
        std::process::exit(1);
    }
}
