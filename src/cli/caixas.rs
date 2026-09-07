//! `optimizer caixas` — mostra os tetos; aplica só com `--aplicar`.

use optimizer::caixas::slice;
use optimizer::diag::maquina;
use optimizer::nucleo::util;

pub(crate) fn caixas_cmd(aplicar: bool, revert: bool) -> Result<(), String> {
    let home = util::home();

    if revert {
        // O revert vem primeiro e sozinho: combinar `--revert` com `--aplicar` seria pedir
        // duas coisas opostas na mesma linha, e adivinhar qual vence é como se apaga o que
        // não se queria apagar.
        if aplicar {
            return Err("`--aplicar` e `--revert` são opostos — peça um de cada vez".into());
        }
        let apagados = slice::reverter(&home)?;
        if apagados.is_empty() {
            println!("nada a reverter (nenhuma caixa desta ferramenta estava aplicada).");
        } else {
            for p in &apagados {
                println!("removido: {}", p.display());
            }
            println!();
            println!("Recarregue o systemd do usuário para valer agora:");
            println!("    systemctl --user daemon-reload");
        }
        return Ok(());
    }

    let m = maquina::detectar();
    if !maquina::cgroup_v2(std::path::Path::new("/sys/fs/cgroup")) {
        return Err(
            "cgroups v2 não está montado — sem ele o systemd ignora os limites em silêncio, e \
             gravar os arquivos daria a impressão de que algo foi feito"
                .into(),
        );
    }
    let ram_mib = m.ram_bytes / (1024 * 1024);
    let caixas = slice::sugerir(ram_mib, m.nucleos);

    println!("CAIXAS SUGERIDAS (RAM {} MiB, {} núcleos)", ram_mib, m.nucleos);
    println!();
    for c in &caixas {
        println!("  {}", c.nome);
        println!("    {}", c.descricao);
        println!(
            "    memória: aperta em {} MiB, mata em {} MiB",
            c.memory_high_mib, c.memory_max_mib
        );
        match c.cpu_quota_pct {
            Some(q) => println!("    cpu    : até {q}% (100% = um núcleo)"),
            None => println!("    cpu    : sem teto (build lento por cota é o oposto do objetivo)"),
        }
        if let Some(w) = c.io_weight {
            println!("    io     : peso {w} (100 é o padrão; menor cede a vez ao interativo)");
        }
        println!();
    }

    if !aplicar {
        println!("Nada foi alterado. Para aplicar:");
        println!("    optimizer caixas --aplicar");
        println!("E para desfazer, a qualquer momento:");
        println!("    optimizer caixas --revert");
        return Ok(());
    }

    let escritos = slice::aplicar(&home, &caixas)?;
    for p in &escritos {
        println!("gravado: {}", p.display());
    }
    println!();
    println!("Para valer agora:");
    println!("    systemctl --user daemon-reload");
    println!();
    println!("Os tetos valem para o que for INICIADO dentro do slice. Ex.:");
    println!("    systemd-run --user --slice=dev-build.slice --scope cargo build");
    println!();
    println!("Desfaz tudo com: optimizer caixas --revert");
    Ok(())
}
