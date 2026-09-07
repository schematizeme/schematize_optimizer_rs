//! `optimizer diag` — mede e sugere. **Não muda nada.**

use optimizer::diag::{gpu, maquina};

/// **O quê:** formata bytes em GiB com uma casa. **Onde:** todo relatório deste arquivo.
fn gib(b: u64) -> String {
    format!("{:.1} GB", b as f64 / 1_073_741_824.0)
}

pub(crate) fn diag_cmd(aguardar: bool) -> Result<(), String> {
    let m = maquina::detectar();
    let g = gpu::detectar();

    println!("MÁQUINA");
    println!("  RAM      : {}", gib(m.ram_bytes));
    println!("  núcleos  : {}", m.nucleos);
    let cg = maquina::cgroup_v2(std::path::Path::new("/sys/fs/cgroup"));
    println!("  cgroups v2: {}", if cg { "sim" } else { "NÃO — as caixas não funcionam sem ele" });

    println!();
    println!("GPU");
    println!("  vendor   : {:?}", g.vendor);
    if let Some(v) = g.vram_bytes {
        println!("  VRAM     : {} (própria da placa)", gib(v));
    }
    match g.gtt_bytes {
        Some(t) => {
            let pct = (t * 100).checked_div(m.ram_bytes).unwrap_or(0);
            println!("  GTT      : {} — {pct}% da sua RAM que a GPU pode mapear", gib(t));
        }
        None => println!("  GTT      : (o driver não expõe)"),
    }

    println!();
    println!("MEMÓRIA COMPARTILHADA DE VÍDEO");
    match gpu::avaliar(&g, m.ram_bytes) {
        gpu::Limitavel::Sim { parametro, atual_mib, sugerido_mib } => {
            println!("  hoje     : {atual_mib} MiB");
            println!("  sugerido : {sugerido_mib} MiB");
            println!("  como     : {parametro}={sugerido_mib} no cmdline do kernel");
            println!();
            println!("  ISTO AINDA NÃO É APLICÁVEL POR AQUI. Mexer no cmdline exige tocar no");
            println!("  bootloader — a única mudança deste app que pode deixar a máquina sem");
            println!("  bootar. Ela só entra com entrada de GRUB ADICIONAL (a original intacta),");
            println!("  backup e um parâmetro por vez. Ver ADR-0011 §5.");
        }
        gpu::Limitavel::Nao(m) => {
            println!("  não dá nesta máquina.");
            println!("  {m}");
        }
    }

    println!();
    println!("CAIXAS DE RECURSO");
    if cg {
        println!("  dá para pôr build, navegador e container cada um no seu teto.");
        println!("  veja o que seria feito: `optimizer caixas`");
    } else {
        println!("  indisponível: cgroups v2 não está montado nesta máquina.");
    }

    if !optimizer::nucleo::desktop::arquivo_desktop(&optimizer::nucleo::util::home()).exists() {
        println!();
        println!("  (este app ainda não está no seu menu de aplicativos:");
        println!("   `optimizer desktop --instalar` põe o ícone lá)");
    }

    if aguardar {
        // O lançador do desktop fecha o terminal quando o processo sai. Sem esta pausa, o
        // clique no ícone seria um piscar.
        println!();
        print!("Enter para fechar… ");
        use std::io::Write;
        let _ = std::io::stdout().flush();
        let mut l = String::new();
        let _ = std::io::stdin().read_line(&mut l);
    }
    Ok(())
}

/// `optimizer desktop` — põe (ou tira) o app do menu de aplicativos.
pub(crate) fn desktop_cmd(instalar: bool, remover: bool) -> Result<(), String> {
    use optimizer::nucleo::desktop;
    if instalar && remover {
        return Err("`--instalar` e `--remover` são opostos — peça um de cada vez".into());
    }
    let home = optimizer::nucleo::util::home();
    if remover {
        let tinha = desktop::remover(&home)?;
        println!("{}", if tinha { "removido do menu." } else { "não estava no menu." });
        return Ok(());
    }
    // O caminho do PRÓPRIO executável: gravar um adivinhado faria o ícone abrir outra coisa
    // (ou nada) em quem instalou fora do lugar padrão.
    let bin = std::env::current_exe()
        .map_err(|e| format!("não descobri o caminho do próprio binário: {e}"))?;
    let p = desktop::instalar(&home, &bin)?;
    println!("instalado: {}", p.display());
    println!("O app agora aparece na lista de programas do sistema.");
    Ok(())
}
