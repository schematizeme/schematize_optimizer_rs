//! `optimizer diag` — mede e sugere. **Não muda nada.**
//!
//! **A saída passa pelo catálogo i18n** (ADR-0014 D8): até então os COMANDOS estavam em inglês
//! e o RELATÓRIO em português, o que faz a interface prometer o que o conteúdo não cumpre.
//!
//! **Chave por PARÁGRAFO, não por linha impressa.** Os blocos explicativos daqui têm três e
//! quatro linhas; uma chave por linha pareceria mais simples e impediria o tradutor de
//! reordenar — e ordem de palavras muda entre idiomas. As chaves carregam o bloco inteiro,
//! com `\n` dentro, e o `println!` recebe o parágrafo pronto.

use optimizer::diag::{gpu, maquina};
use optimizer::nucleo::i18n::{t, tf};

/// **O quê:** formata bytes em GiB com uma casa. **Onde:** todo relatório deste arquivo.
fn gib(b: u64) -> String {
    format!("{:.1} GB", b as f64 / 1_073_741_824.0)
}

/// **O quê:** mede a máquina e diz o que dá para melhorar. **Não muda nada.**
///
/// **Onde:** `schematize-optimizer diag`, que é o comando padrão e o que o ícone abre.
///
/// **`json` sai ANTES de qualquer `println!` humano** e retorna: uma linha de relatório
/// impressa junto tornaria o documento inválido, e a janela mostraria tela vazia sem dizer
/// por quê. O `--json` também é `conflicts_with` do `--wait` no clap — esperar tecla numa
/// saída de máquina travaria a janela para sempre.
pub(crate) fn diag_cmd(wait: bool, json: bool) -> Result<(), String> {
    let m = maquina::detectar();
    let g = gpu::detectar();
    let cg = maquina::cgroup_v2(std::path::Path::new("/sys/fs/cgroup"));
    let fora_do_menu =
        !optimizer::nucleo::desktop::arquivo_desktop(&optimizer::nucleo::util::home()).exists();

    if json {
        super::saidajson::diag(&m, &g, cg, fora_do_menu);
        return Ok(());
    }

    println!("{}", t("diag.machine_header"));
    println!("{}", tf("diag.machine_ram", &[("value", &gib(m.ram_bytes))]));
    println!("{}", tf("diag.machine_cores", &[("value", &m.nucleos.to_string())]));
    println!(
        "{}",
        tf(
            "diag.machine_cgroups",
            &[("value", &t(if cg { "diag.cgroups_yes" } else { "diag.cgroups_no" }))]
        )
    );

    println!();
    println!("{}", t("diag.gpu_header"));
    println!("{}", tf("diag.gpu_vendor", &[("value", &format!("{:?}", g.vendor))]));
    if let Some(v) = g.vram_bytes {
        println!("{}", tf("diag.gpu_vram", &[("value", &gib(v))]));
    }
    match g.gtt_bytes {
        Some(gtt) => {
            let pct = (gtt * 100).checked_div(m.ram_bytes).unwrap_or(0);
            println!("{}", tf("diag.gpu_gtt", &[("value", &gib(gtt)), ("pct", &pct.to_string())]));
        }
        None => println!("{}", t("diag.gpu_gtt_unknown")),
    }

    println!();
    println!("{}", t("diag.vram_header"));
    match gpu::avaliar(&g, m.ram_bytes) {
        gpu::Limitavel::Sim { parametro, atual_mib, sugerido_mib } => {
            let sug = sugerido_mib.to_string();
            println!("{}", tf("diag.vram_now", &[("value", &atual_mib.to_string())]));
            println!("{}", tf("diag.vram_suggested", &[("value", &sug)]));
            println!("{}", tf("diag.vram_how", &[("param", parametro), ("value", &sug)]));
            println!();
            // O bloco de QUATRO linhas que explica por que isto não se aplica sozinho. Uma
            // chave só: é um argumento, e argumento partido em quatro chaves não se traduz.
            println!("{}", t("diag.vram_not_applicable"));
        }
        gpu::Limitavel::Nao(motivo) => {
            println!("{}", t("diag.vram_impossible"));
            println!("  {}", t(&motivo));
        }
    }

    println!();
    println!("{}", t("diag.boxes_header"));
    println!("{}", t(if cg { "diag.boxes_available" } else { "diag.boxes_unavailable" }));

    if fora_do_menu {
        println!();
        println!("{}", t("diag.not_in_menu"));
    }

    if wait {
        esperar_tecla();
    }
    Ok(())
}

/// **O quê:** segura a janela até uma tecla. **Onde:** `diag --wait`, chamado pelo `.desktop`.
///
/// O lançador do desktop fecha o terminal quando o processo sai. Sem esta pausa, o clique no
/// ícone seria um piscar — e o §37.48 chama isso de bug do software, não de erro de quem
/// clicou.
fn esperar_tecla() {
    use std::io::Write;
    println!();
    print!("{} ", t("common.press_enter"));
    let _ = std::io::stdout().flush();
    let mut l = String::new();
    let _ = std::io::stdin().read_line(&mut l);
}

/// `optimizer desktop` — põe (ou tira) o app do menu de aplicativos.
pub(crate) fn desktop_cmd(install: bool, remove: bool) -> Result<(), String> {
    use optimizer::nucleo::desktop;
    if install && remove {
        return Err(t("desktop.opposites"));
    }
    let home = optimizer::nucleo::util::home();
    if remove {
        let tinha = desktop::remover(&home)?;
        println!("{}", t(if tinha { "desktop.removed" } else { "desktop.was_absent" }));
        return Ok(());
    }
    // O caminho do PRÓPRIO executável: gravar um adivinhado faria o ícone abrir outra coisa
    // (ou nada) em quem instalou fora do lugar padrão.
    let bin = std::env::current_exe()
        .map_err(|e| tf("desktop.no_self_path", &[("error", &e.to_string())]))?;
    let p = desktop::instalar(&home, &bin)?;
    println!("{}", tf("desktop.installed", &[("path", &p.display().to_string())]));
    println!("{}", t("desktop.now_listed"));
    Ok(())
}
