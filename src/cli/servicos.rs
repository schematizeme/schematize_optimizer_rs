//! `optimizer services` — o que sobe no boot e o pouco que dá para desligar.
//!
//! **Só relata.** Desabilitar serviço é onde se quebra máquina, e o ADR-0011 reserva o
//! `--aplicar` automático às caixas. Aqui a ferramenta mostra, explica e **entrega o comando**
//! — quem decide é a pessoa, item a item.

use optimizer::nucleo::i18n::{t, tf};
use optimizer::nucleo::util;
use optimizer::servicos;

/// **O quê:** roda um comando e devolve o stdout, ou vazio se falhar.
///
/// **Onde:** a leitura do `systemctl` e do `systemd-analyze`. Falha vira vazio de propósito:
/// numa máquina sem systemd o comando não existe, e isso é "não há o que relatar" — não um
/// erro que derrube o app.
fn saida(cmd: &str, args: &[&str]) -> String {
    util::run(cmd, args).unwrap_or_default()
}

pub(crate) fn servicos_cmd() -> Result<(), String> {
    let hab = servicos::parse_habilitados(&saida(
        "systemctl",
        &["list-unit-files", "--state=enabled", "--type=service", "--no-legend", "--no-pager"],
    ));
    if hab.is_empty() {
        return Err(t("services.no_systemd"));
    }
    let custos = servicos::parse_blame(&saida("systemd-analyze", &["blame", "--no-pager"]));
    let lista = servicos::cruzar(&hab, &custos);

    println!("{}", tf("services.header", &[("count", &lista.len().to_string())]));
    println!();
    for s in &lista {
        let custo = match s.custo_s {
            Some(c) => format!("{c:>6.2}s"),
            None => "     —".to_string(),
        };
        let marca = if servicos::vale_a_pena(s) { " ←" } else { "" };
        println!("  {custo}  {}{marca}", s.unidade);
    }

    let recomendados: Vec<_> = lista.iter().filter(|s| servicos::vale_a_pena(s)).collect();
    println!();
    if recomendados.is_empty() {
        println!("{}", t("services.nothing_to_suggest"));
        return Ok(());
    }

    let ganho: f64 = recomendados.iter().filter_map(|s| s.custo_s).sum();
    println!("{}", tf("services.can_disable", &[("gain", &format!("{ganho:.1}"))]));
    println!();
    for s in &recomendados {
        let seg = s.seguro.expect("vale_a_pena garante");
        println!("  {}", s.unidade);
        let custo_s = format!("{:.2}", s.custo_s.unwrap_or(0.0));
        println!("{}", tf("services.costs", &[("seconds", &custo_s)]));
        println!("{}", tf("services.does", &[("value", &t(seg.o_que_faz))]));
        println!("{}", tf("services.why_safe", &[("value", &t(seg.por_que_da))]));
        // O custo vem SEMPRE, e vem antes do comando: quem lê tem de poder discordar.
        println!("{}", tf("services.what_you_lose", &[("value", &t(seg.o_que_se_perde))]));
        println!("{}", tf("services.disable_cmd", &[("unit", &s.unidade)]));
        println!("{}", tf("services.enable_cmd", &[("unit", &s.unidade)]));
        println!();
    }
    println!("{}", t("services.we_do_not_touch"));
    Ok(())
}
