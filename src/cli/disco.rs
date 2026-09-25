//! `optimizer disco` — o que está enchendo o disco e pode ser refeito.
//!
//! **Onde:** o subcomando, e a aba Disco da janela por `--json`.
//!
//! ## Por que isto vive AQUI (E3 do ADR-0018)
//!
//! Veio do app principal, onde estava por acidente de história: o módulo nasceu junto com a tela
//! que o mostrava. A pergunta que ele responde — *"o que esta máquina está desperdiçando?"* — é
//! a mesma que o `diag` e as `caixas` já faziam, palavra por palavra do ADR-0011.
//!
//! ## APAGAR passa por TERMINAL, sempre (D6 do ADR-0016)
//!
//! A limpeza apaga arquivo. Progresso, `Ctrl-C` e erro copiável vêm do terminal de verdade, e
//! desenhá-los numa janela seria uma aproximação pior. A janela **monta o comando e abre o
//! terminal** — ela não apaga nada, e há teste disso do lado dela.
//!
//! A confirmação também é daqui, não da janela: quem vê a lista do que vai sumir é quem digita
//! `s`.

use optimizer::disco::{self, docker, tamanho::legivel};
use optimizer::nucleo::util;
use std::io::Write;

/// Corte de ruído: abaixo disto não vale nem listar.
///
/// **50 MiB** porque a lista existe para responder "o que está enchendo o disco", e um diretório
/// de 3 MiB nunca é a resposta — mas entope a tela e esconde o que é.
const MINIMO: u64 = 50 * 1024 * 1024;

/// **O quê:** os diretórios a varrer — os passados em `--dir`, ou os cadastrados.
///
/// **Onde:** [`disco_cmd`].
///
/// **`--dir` ganha quando passado:** quem o passou quer AQUELE diretório. Sem ele, cai na lista
/// cadastrada no `config.json` do ecossistema, que é só lida — este app nunca a escreve.
fn alvos(dirs: Vec<String>) -> Vec<String> {
    if !dirs.is_empty() {
        return dirs;
    }
    let p = util::dados_dir().join("config.json");
    let Ok(txt) = std::fs::read_to_string(p) else { return Vec::new() };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&txt) else { return Vec::new() };
    v.get("dev_dirs")
        .and_then(|x| x.as_array())
        .map(|a| a.iter().filter_map(|x| x.as_str()).map(String::from).collect())
        .unwrap_or_default()
}

/// **O quê:** o inventário, humano ou de máquina.
///
/// **Onde:** `main`.
///
/// **Uma leitura só, dois caminhos de saída.** Varrer duas vezes — uma por caminho — abriria a
/// porta para os dois discordarem, e é o defeito que este ecossistema já pagou.
pub(crate) fn disco_cmd(min_dias: u64, dirs: Vec<String>, json: bool) -> Result<(), String> {
    let devs = alvos(dirs);
    if !json && devs.is_empty() {
        println!("nenhum diretório de dev cadastrado.");
        println!("  passe `--dir <caminho>`, ou cadastre um pela janela do schematize.");
    }
    if !json {
        println!("varrendo {} diretório(s)…", devs.len());
    }
    let achados: Vec<_> = disco::inventario(&devs, MINIMO)
        .into_iter()
        .filter(|a| a.dias_parado >= min_dias)
        .collect();
    let uso_docker = docker::uso();

    if json {
        println!("{}", super::saidajson::disco(&achados, &uso_docker));
        return Ok(());
    }

    println!("\n\x1b[1mPOR DISCO\x1b[0m (é o que enche)");
    for (m, b) in disco::por_montagem(&achados) {
        println!("  {:>10}  {}", legivel(b), m.display());
    }
    println!("\n\x1b[1mPOR TIPO\x1b[0m");
    for (t, b) in disco::por_tipo(&achados) {
        let custo = if t.custa_rede() { "baixa de novo" } else { "compila de novo" };
        println!("  {:>10}  {:<16} ({custo})", legivel(b), t.rotulo());
    }
    println!("\n\x1b[1mMAIORES\x1b[0m");
    for a in achados.iter().take(15) {
        println!(
            "  {:>10}  {:<16} {} dias  {}",
            legivel(a.bytes),
            a.tipo.rotulo(),
            a.dias_parado,
            a.caminho.display()
        );
    }
    let total: u64 = achados.iter().map(|a| a.bytes).sum();
    println!("\ntotal recuperável nos projetos: \x1b[1m{}\x1b[0m", legivel(total));
    imprime_docker(&uso_docker);
    println!(
        "\nlimpar: `schematize-optimizer disco-clean --min-dias 30`   \
         (docker: `schematize-optimizer disco-docker`)"
    );
    Ok(())
}

/// **O quê:** o bloco do Docker.
///
/// **Onde:** [`disco_cmd`] e [`docker_cmd`].
///
/// **Silencioso sem docker:** máquina sem docker não deve ver erro de docker. É ausência de
/// assunto, não falha.
fn imprime_docker(uso: &[docker::Categoria]) {
    if uso.is_empty() {
        return;
    }
    println!("\n\x1b[1mDOCKER\x1b[0m");
    let mut rec = 0u64;
    for c in uso {
        println!(
            "  {:>10}  {:<16} (recuperável: {})",
            legivel(c.bytes),
            c.tipo,
            legivel(c.recuperavel)
        );
        rec += c.recuperavel;
    }
    println!("  recuperável no docker: \x1b[1m{}\x1b[0m", legivel(rec));
}

/// **O quê:** apaga os achados que casam com os filtros, mostrando a lista ANTES.
///
/// **Onde:** `schematize-optimizer disco-clean`, e é para aqui que a janela abre o terminal.
///
/// **A lista vem antes da pergunta, sempre.** Confirmar sem ver o que some é assinar em branco.
pub(crate) fn limpar_cmd(
    min_dias: u64,
    dirs: Vec<String>,
    tipo: Option<String>,
    montagem: Option<String>,
    yes: bool,
) -> Result<(), String> {
    let devs = alvos(dirs);
    let alvos: Vec<_> = disco::inventario(&devs, MINIMO)
        .into_iter()
        .filter(|a| a.dias_parado >= min_dias)
        .filter(|a| tipo.as_deref().is_none_or(|t| a.tipo.rotulo().contains(t)))
        .filter(|a| montagem.as_deref().is_none_or(|m| a.montagem.starts_with(m)))
        .collect();

    if alvos.is_empty() {
        println!("nada a limpar com esses filtros.");
        return Ok(());
    }
    let total: u64 = alvos.iter().map(|a| a.bytes).sum();
    println!("vou apagar {} item(ns), liberando ~{}:", alvos.len(), legivel(total));
    for a in &alvos {
        println!(
            "  {:>10}  {:<16} {}  \x1b[2m({})\x1b[0m",
            legivel(a.bytes),
            a.tipo.rotulo(),
            a.caminho.display(),
            a.refaz
        );
    }
    if !yes && !confirma("apagar? [s/N] ") {
        println!("cancelado.");
        return Ok(());
    }
    let mut liberado = 0u64;
    for a in &alvos {
        // Piso 4: a falha de UM item não interrompe os outros, e não some da tela.
        match disco::remover(a, &devs) {
            Ok(b) => liberado += b,
            Err(e) => println!("  \x1b[33mfalhou\x1b[0m {e}"),
        }
    }
    println!("liberado: \x1b[1m{}\x1b[0m", legivel(liberado));
    Ok(())
}

/// **O quê:** as podas do Docker; com `--podar <rótulo>`, executa uma.
///
/// **Onde:** `schematize-optimizer disco-docker`.
pub(crate) fn docker_cmd(podar: Option<String>, yes: bool) -> Result<(), String> {
    if !docker::disponivel() {
        return Err("docker não está disponível (não instalado ou o daemon está parado).".into());
    }
    let Some(rotulo) = podar else {
        imprime_docker(&docker::uso());
        println!("\npodas disponíveis:");
        for (r, _, destrutiva) in docker::podas() {
            let marca = if destrutiva { "  \x1b[31m← apaga DADOS\x1b[0m" } else { "" };
            println!("  schematize-optimizer disco-docker --podar \"{r}\"{marca}");
        }
        return Ok(());
    };
    let destrutiva = docker::podas().into_iter().any(|(r, _, d)| r == rotulo && d);
    if destrutiva {
        // **Volume é DADO, não build.** Confirmação SEMPRE, mesmo com `--yes`: um `--yes`
        // digitado para limpar cache de build não pode levar o banco de dev junto.
        println!("\x1b[31mEsta poda APAGA DADOS\x1b[0m (volumes: banco de dev, uploads de teste).");
        if !confirma("tem certeza? digite 'sim' para confirmar: ") {
            println!("cancelado.");
            return Ok(());
        }
    } else if !yes && !confirma(&format!("podar \"{rotulo}\"? [s/N] ")) {
        println!("cancelado.");
        return Ok(());
    }
    println!("{}", docker::podar(&rotulo)?.trim());
    Ok(())
}

/// **O quê:** pergunta no terminal. Aceita `s`/`sim`/`y`/`yes`.
///
/// **Onde:** as duas confirmações deste arquivo.
///
/// **Qualquer outra coisa é NÃO**, inclusive erro de leitura: quando não dá para saber o que a
/// pessoa quis, não apagar é o único default seguro.
fn confirma(prompt: &str) -> bool {
    print!("{prompt}");
    let _ = std::io::stdout().flush();
    let mut l = String::new();
    if std::io::stdin().read_line(&mut l).is_err() {
        return false;
    }
    matches!(l.trim().to_lowercase().as_str(), "s" | "sim" | "y" | "yes")
}
