//! `optimizer jaula` — um teto duro de memória POR EXECUÇÃO.
//!
//! **O quê:** `estimar` (puro), `rodar` (executa jaulado) e `diag` (backend + o que falta).
//! A lógica vive em [`optimizer::jaula`]; aqui só se lê argumento e se imprime.
//!
//! **Onde:** `schematize-optimizer jaula …`. Diferente de `limits` (slices globais), cada
//! comando ganha o próprio cgroup: estourou, morre SÓ ele.
//!
//! **Stdout é do filho no modo humano** (o comando jaulado herda a saída); as mensagens do
//! optimizer vão para o stderr. No `--json`, o stdout é só o documento e o filho vai ao stderr.

use super::args::JaulaCmd;
use optimizer::jaula::{self, Desfecho, Pedido};
use optimizer::nucleo::i18n::{t, tf};
use schematize_plataforma::jaula::Backend;

/// **O quê:** despacha o subcomando de `jaula`. **Onde:** `main`.
pub(crate) fn jaula_cmd(sub: JaulaCmd) -> Result<(), String> {
    match sub {
        JaulaCmd::Estimar { tipo, json } => estimar_cmd(tipo.as_deref(), json),
        JaulaCmd::Rodar { classe, mem_mib, timeout_s, nome, adr, json, cmd } => {
            let pedido =
                Pedido { classe: classe.as_deref(), mem_mib, timeout_s, adr: adr.as_deref() };
            rodar_cmd(&pedido, nome.as_deref(), json, &cmd)
        }
        JaulaCmd::Diag { json } => diag_cmd(json),
    }
}

/// **O quê:** traduz uma (chave, args) em texto. **Onde:** mensagens deste arquivo.
fn msg(chave: &str, args: &[(&'static str, String)]) -> String {
    let refs: Vec<(&str, &str)> = args.iter().map(|(k, v)| (*k, v.as_str())).collect();
    tf(chave, &refs)
}

/// **O quê:** `jaula estimar`: PURO, não executa nada. **Onde:** [`jaula_cmd`].
fn estimar_cmd(tipo: Option<&str>, json: bool) -> Result<(), String> {
    let alvo = tipo.and_then(jaula::tipo_de_slug);
    let linhas = jaula::estimativas(alvo);
    if json {
        println!("{}", super::saidajson::jaula_estimar(&linhas));
        return Ok(());
    }
    println!("{}", t("jaula.estimar.header"));
    for l in &linhas {
        let a = [
            ("tipo", l.tipo.to_string()),
            ("classe", l.classe.to_string()),
            ("max", l.mem_max_mib.to_string()),
            ("high", l.mem_high_mib.to_string()),
            ("jobs", l.jobs.to_string()),
        ];
        println!("{}", msg("jaula.estimar.linha", &a));
    }
    println!();
    println!("{}", t("jaula.estimar.rodape"));
    Ok(())
}

/// **O quê:** `jaula rodar`: valida, executa jaulado e sai com o código do desfecho. Pedido
/// inválido volta como `Err` (exit 1) com a mensagem acionável.
/// **Onde:** [`jaula_cmd`].
fn rodar_cmd(p: &Pedido, nome: Option<&str>, json: bool, cmd: &[String]) -> Result<(), String> {
    let orcamento = jaula::montar_orcamento(p).map_err(|e| {
        let (chave, args) = e.mensagem();
        msg(chave, &args)
    })?;
    let nome = nome.map_or_else(|| format!("optimizer-{}", std::process::id()), str::to_string);
    let (d, backend) = jaula::rodar(&orcamento, cmd, &nome, json);
    if json {
        println!("{}", super::saidajson::jaula_rodar(&d, &nome, jaula::slug_backend(backend)));
    } else {
        explicar(&d);
    }
    if d.codigo != 0 {
        std::process::exit(d.codigo);
    }
    Ok(())
}

/// **O quê:** imprime no stderr a mensagem do desfecho (nada quando concluiu).
/// **Onde:** [`rodar_cmd`], modo humano.
fn explicar(d: &Desfecho) {
    if let Some(chave) = d.chave {
        eprintln!("{}", msg(chave, &d.args));
    }
}

/// **O quê:** `jaula diag`: backend detectado e o que falta para o forte. Só leitura.
/// **Onde:** [`jaula_cmd`].
fn diag_cmd(json: bool) -> Result<(), String> {
    let backend = schematize_plataforma::jaula::detectar_backend();
    let fatos = jaula::coletar();
    if json {
        println!("{}", super::saidajson::jaula_diag(jaula::slug_backend(backend), &fatos));
        return Ok(());
    }
    println!("{}", t("jaula.diag.header"));
    let chave = match backend {
        Backend::SystemdScope => "jaula.diag.backend_systemd",
        Backend::Prlimit => "jaula.diag.backend_prlimit",
        Backend::Nenhum => "jaula.diag.backend_nenhum",
    };
    println!("{}", t(chave));
    let faltas = jaula::faltas(&fatos);
    if faltas.is_empty() {
        println!("{}", t("jaula.diag.nada_falta"));
        return Ok(());
    }
    println!("{}", t("jaula.diag.faltam"));
    for f in faltas {
        println!("{}", t(&format!("jaula.diag.falta.{f}")));
    }
    Ok(())
}
