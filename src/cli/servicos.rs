//! `optimizer servicos` — o que sobe no boot e o pouco que dá para desligar.
//!
//! **Só relata.** Desabilitar serviço é onde se quebra máquina, e o ADR-0011 reserva o
//! `--aplicar` automático às caixas. Aqui a ferramenta mostra, explica e **entrega o comando**
//! — quem decide é a pessoa, item a item.

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
        return Err(
            "não consegui listar os serviços habilitados — esta máquina tem systemd?".into()
        );
    }
    let custos = servicos::parse_blame(&saida("systemd-analyze", &["blame", "--no-pager"]));
    let lista = servicos::cruzar(&hab, &custos);

    println!("O QUE SOBE NO BOOT ({} serviços habilitados)", lista.len());
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
        println!("NADA A SUGERIR");
        println!("  Os serviços da allowlist ou não estão habilitados aqui, ou não custam");
        println!("  nada mensurável neste boot. Mexer no sistema por ganho zero é o oposto");
        println!("  de otimizar.");
        return Ok(());
    }

    let ganho: f64 = recomendados.iter().filter_map(|s| s.custo_s).sum();
    println!("DÁ PARA DESLIGAR COM SEGURANÇA (ganho estimado: {ganho:.1}s de boot)");
    println!();
    for s in &recomendados {
        let seg = s.seguro.expect("vale_a_pena garante");
        println!("  {}", s.unidade);
        println!("    custa       : {:.2}s neste boot", s.custo_s.unwrap_or(0.0));
        println!("    o que faz   : {}", seg.o_que_faz);
        println!("    por que dá  : {}", seg.por_que_da);
        // O custo vem SEMPRE, e vem antes do comando: quem lê tem de poder discordar.
        println!("    o que perde : {}", seg.o_que_se_perde);
        println!("    desligar    : sudo systemctl disable --now {}", s.unidade);
        println!("    voltar      : sudo systemctl enable --now {}", s.unidade);
        println!();
    }
    println!("Esta ferramenta NÃO desabilita nada sozinha. Desabilitar serviço é onde se");
    println!("quebra máquina, e a causa fica longe do sintoma — leia o que se perde, decida");
    println!("item a item, e rode o comando você.");
    Ok(())
}
