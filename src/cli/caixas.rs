//! `optimizer limits` — mostra os tetos; aplica só com `--apply`.
//!
//! A saída passa pelo catálogo i18n (ADR-0014 D8), com chave por PARÁGRAFO.

use optimizer::caixas::slice;
use optimizer::diag::maquina;
use optimizer::nucleo::i18n::{t, tf};
use optimizer::nucleo::util;

pub(crate) fn caixas_cmd(apply: bool, revert: bool) -> Result<(), String> {
    let home = util::home();

    if revert {
        // O revert vem primeiro e sozinho: combinar `--revert` com `--apply` seria pedir duas
        // coisas opostas na mesma linha, e adivinhar qual vence é como se apaga o que não se
        // queria apagar.
        if apply {
            return Err(t("limits.opposites"));
        }
        return reverter(&home);
    }

    let m = maquina::detectar();
    if !maquina::cgroup_v2(std::path::Path::new("/sys/fs/cgroup")) {
        return Err(t("limits.no_cgroups"));
    }
    let ram_mib = m.ram_bytes / (1024 * 1024);
    let caixas = slice::sugerir(ram_mib, m.nucleos);

    println!(
        "{}",
        tf("limits.header", &[("ram", &ram_mib.to_string()), ("cores", &m.nucleos.to_string())])
    );
    println!();
    for c in &caixas {
        println!("  {}", c.nome);
        // `descricao` é CHAVE de catálogo, não prosa — traduzir aqui é o que impede
        // `box.build` de aparecer cru na tela. O varredor de saída não pega isto (é um `{}`,
        // formatação legítima), então a garantia é o teste de ponta a ponta abaixo.
        println!("    {}", t(&c.descricao));
        println!(
            "{}",
            tf(
                "limits.memory",
                &[("high", &c.memory_high_mib.to_string()), ("max", &c.memory_max_mib.to_string())]
            )
        );
        match c.cpu_quota_pct {
            Some(q) => println!("{}", tf("limits.cpu", &[("pct", &q.to_string())])),
            None => println!("{}", t("limits.cpu_none")),
        }
        if let Some(w) = c.io_weight {
            println!("{}", tf("limits.io", &[("weight", &w.to_string())]));
        }
        println!();
    }

    if !apply {
        println!("{}", t("limits.dry_run"));
        return Ok(());
    }

    let escritos = slice::aplicar(&home, &caixas)?;
    for p in &escritos {
        println!("{}", tf("limits.written", &[("path", &p.display().to_string())]));
    }
    println!();
    println!("{}", t("limits.after_apply"));
    Ok(())
}

/// **O quê:** desfaz o que este app aplicou. **Onde:** `limits --revert`.
///
/// **Nada a reverter é RESULTADO, não erro:** quem roda `--revert` numa máquina limpa quer
/// saber que ela está limpa, e um erro ali mandaria procurar problema onde não há.
fn reverter(home: &std::path::Path) -> Result<(), String> {
    let apagados = slice::reverter(home)?;
    if apagados.is_empty() {
        println!("{}", t("limits.nothing_to_revert"));
        return Ok(());
    }
    for p in &apagados {
        println!("{}", tf("limits.removed", &[("path", &p.display().to_string())]));
    }
    println!();
    println!("{}", t("limits.reload_systemd"));
    Ok(())
}
