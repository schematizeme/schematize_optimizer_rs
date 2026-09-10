//! A superfície da CLI do Optimizer.
//!
//! **O quê:** a árvore de comandos do clap. Só definição.
//!
//! ## O padrão é RELATAR, e isso aparece na forma dos comandos
//!
//! `optimizer diag` e `optimizer limits` **não mudam nada**. Quem muda é `--aplicar`, e ele
//! existe só onde o ADR-0011 permite: nas caixas. É o `disco` mostrando antes de apagar,
//! levado a sério para um alvo que não se recria.

use clap::{Parser, Subcommand};

/// `optimizer` — deixa a máquina de dev previsível.
#[derive(Parser)]
#[command(
    name = "schematize-optimizer",
    version,
    about = "schematize optimizer — measures the dev environment and caps each software",
    long_about = "Suggests by default. Only changes what it knows how to undo, and `--revert` undoes it.\n\
                  Works standalone; integrates with schematize when both are present."
)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) cmd: Cmd,
}

#[derive(Subcommand)]
pub(crate) enum Cmd {
    /// Measure the machine and say what can be improved. Changes NOTHING. This is what the
    /// icon opens.
    Diag {
        /// Wait for a keypress at the end. The desktop launcher uses this — without it the
        /// terminal would close instantly and the click would look like it did nothing.
        #[arg(long)]
        wait: bool,
        /// Machine-readable output. Stable keys, never translated — this is what the window
        /// reads. Human output goes through the i18n catalog and is NOT a contract.
        #[arg(long, conflicts_with = "wait")]
        json: bool,
    },
    /// Per-software resource ceilings (build, browser, container), via systemd slices.
    Limits {
        /// Write the slices. Without this, only shows what it would do.
        #[arg(long)]
        apply: bool,
        /// Delete the slices THIS tool generated — and only those.
        #[arg(long)]
        revert: bool,
        /// Machine-readable output: what is suggested AND what is already on disk, so the
        /// window can show what changes before changing it. Reads only; never writes.
        #[arg(long, conflicts_with_all = ["apply", "revert"])]
        json: bool,
    },
    /// What starts at boot, what it costs, and the little that is safe to turn off.
    Services {
        /// Machine-readable output. Stable keys, never translated.
        #[arg(long)]
        json: bool,
    },
    /// Icon and application-menu entry — so the app opens without schematize.
    Desktop {
        /// Install (the default when no flag is given).
        #[arg(long)]
        install: bool,
        /// Remove the menu entry.
        #[arg(long)]
        remove: bool,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    /// Serializa a ÁRVORE INTEIRA de comandos num texto determinístico.
    ///
    /// **O quê:** para cada comando e subcomando, em ordem alfabética — o caminho, os
    /// aliases, se é oculto, e cada argumento com nome longo/curto, se é obrigatório, se
    /// recebe valor e qual o default. Nada de texto de ajuda: descrição muda com revisão de
    /// prosa e não é contrato; o que a pessoa DIGITA é.
    ///
    /// **Onde:** [`superficie_da_cli_nao_mudou`], contra um snapshot commitado.
    fn superficie(c: &clap::Command, caminho: &str, out: &mut Vec<String>) {
        let nome = if caminho.is_empty() {
            c.get_name().to_string()
        } else {
            format!("{caminho} {}", c.get_name())
        };
        let mut aliases: Vec<_> = c.get_all_aliases().collect();
        aliases.sort_unstable();
        out.push(format!(
            "CMD {nome}{}{}",
            if aliases.is_empty() {
                String::new()
            } else {
                format!(" aliases=[{}]", aliases.join(","))
            },
            if c.is_hide_set() { " (oculto)" } else { "" }
        ));
        // `SOBRE` é a descrição — vai pro arquivo (é dela que o índice de funcionalidades
        // se alimenta) mas FICA DE FORA da comparação: prosa muda em revisão de texto e não
        // é contrato. Quem quebra script é a linha `CMD`/`ARG`, não o `about`.
        if let Some(sobre) = c.get_about() {
            let t = sobre.to_string();
            out.push(format!("  SOBRE {}", t.lines().next().unwrap_or("").trim()));
        }

        let mut args: Vec<String> = c
            .get_arguments()
            .map(|a| {
                let longo = a.get_long().map(|l| format!("--{l}")).unwrap_or_default();
                let curto = a.get_short().map(|s| format!(" -{s}")).unwrap_or_default();
                let val = if a.get_num_args().map(|n| n.takes_values()).unwrap_or(false) {
                    " <valor>"
                } else {
                    ""
                };
                let obrig = if a.is_required_set() { " OBRIGATORIO" } else { "" };
                format!("  ARG {:<20} {longo}{curto}{val}{obrig}", a.get_id().to_string())
            })
            .collect();
        args.sort();
        out.extend(args);

        let mut subs: Vec<&clap::Command> = c.get_subcommands().collect();
        subs.sort_by_key(|s| s.get_name());
        for s in subs {
            superficie(s, &nome, out);
        }
    }

    /// A superfície da CLI é CONTRATO com quem escreveu script, hook e documentação.
    ///
    /// **Onde:** roda a cada `cargo test`, e existe pra que refatorar este módulo seja
    /// seguro — o `args.rs` passou de 780 linhas e precisou ser partido em submódulos por
    /// domínio, e sem esta prova o corte seria confiança, não verificação.
    ///
    /// **Se este teste falhar** e a mudança for INTENCIONAL (comando novo, flag nova),
    /// regenere com `OPTIMIZER_REGRAVA_SUPERFICIE=1 cargo test superficie_da_cli` e leia o
    /// diff **linha por linha** antes de commitar: cada linha some é um script de alguém
    /// quebrando. Se foi acidente de refatoração, o teste acabou de fazer o trabalho dele.
    #[test]
    fn superficie_da_cli_nao_mudou() {
        let mut linhas = Vec::new();
        superficie(&Cli::command(), "", &mut linhas);
        let atual = linhas.join("\n") + "\n";

        let snap = std::path::Path::new("tests/superficie-cli.txt");
        if std::env::var_os("OPTIMIZER_REGRAVA_SUPERFICIE").is_some() {
            std::fs::write(snap, &atual).expect("gravar o snapshot");
            return;
        }
        let esperado = std::fs::read_to_string(snap).expect(
            "tests/superficie-cli.txt ausente — gere com \
             OPTIMIZER_REGRAVA_SUPERFICIE=1 cargo test superficie_da_cli",
        );
        // A ASSERÇÃO é só sobre o contrato: `CMD` e `ARG`. As linhas `SOBRE` viajam no
        // arquivo pra alimentar o índice de funcionalidades, e mudam livremente com revisão
        // de prosa — descrição não quebra o script de ninguém.
        let contrato = |t: &str| -> Vec<String> {
            t.lines().filter(|l| !l.trim_start().starts_with("SOBRE ")).map(String::from).collect()
        };
        if contrato(&atual) == contrato(&esperado) {
            // Só a prosa mudou: regrava sem reprovar.
            if atual != esperado {
                std::fs::write(snap, &atual).expect("regravar a prosa do snapshot");
            }
            return;
        }
        // Diff legível: a primeira divergência é o que a pessoa precisa ver.
        let (a, e) = (contrato(&atual), contrato(&esperado));
        let sumiram: Vec<_> = e.iter().filter(|l| !a.contains(l)).collect();
        let surgiram: Vec<_> = a.iter().filter(|l| !e.contains(l)).collect();
        panic!(
            "a superfície da CLI MUDOU.\n\nsumiram ({}) — cada uma é um script de alguém \
             quebrando:\n  {}\n\nsurgiram ({}):\n  {}\n\nSe foi intencional: \
             OPTIMIZER_REGRAVA_SUPERFICIE=1 cargo test superficie_da_cli",
            sumiram.len(),
            sumiram.iter().map(|s| s.to_string()).collect::<Vec<_>>().join("\n  "),
            surgiram.len(),
            surgiram.iter().map(|s| s.to_string()).collect::<Vec<_>>().join("\n  "),
        );
    }
}
