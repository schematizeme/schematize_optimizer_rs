//! A superfície da CLI do Optimizer.
//!
//! **O quê:** a árvore de comandos do clap. Só definição.
//!
//! ## O padrão é RELATAR, e isso aparece na forma dos comandos
//!
//! `optimizer diag` e `optimizer caixas` **não mudam nada**. Quem muda é `--aplicar`, e ele
//! existe só onde o ADR-0011 permite: nas caixas. É o `disco` mostrando antes de apagar,
//! levado a sério para um alvo que não se recria.

use clap::{Parser, Subcommand};

/// `optimizer` — deixa a máquina de dev previsível.
#[derive(Parser)]
#[command(
    name = "optimizer",
    version,
    about = "schematize optimizer — mede o ambiente de dev e põe cada software no seu teto",
    long_about = "Sugere por padrão. Só muda o que sabe desfazer, e o que muda, `--revert` volta.\n\
                  Funciona sozinho; integra-se ao schematize quando os dois convivem."
)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) cmd: Cmd,
}

#[derive(Subcommand)]
pub(crate) enum Cmd {
    /// Mede a máquina e diz o que dá para melhorar. NÃO muda nada.
    Diag,
    /// Tetos de recurso por software (build, navegador, container), via systemd.
    Caixas {
        /// Grava os slices. Sem isto, só mostra o que faria.
        #[arg(long)]
        aplicar: bool,
        /// Apaga os slices que ESTA ferramenta gerou — e só eles.
        #[arg(long)]
        revert: bool,
    },
}
