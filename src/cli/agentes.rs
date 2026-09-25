//! `optimizer agentes` — quantos agentes do Claude esta máquina aguenta em PARALELO.
//!
//! **Onde:** o subcomando, e a janela por `--json`.
//!
//! ## Por que isto vive AQUI (E3 do ADR-0018)
//!
//! Veio do app principal, onde nasceu junto com o overdev — que era quem precisava do número. Mas
//! a pergunta não é do overdev: é **"o que esta máquina aguenta?"**, medida em CPU, RAM e load.
//! É a pergunta deste app, e o ADR-0011 a escreve com essas palavras.
//!
//! ## Conta o que JÁ está rodando, e isso é o ponto
//!
//! O orçamento é **da máquina**, não de uma invocação. Um número que ignorasse os `claude` já
//! abertos diria "cabem 4" para quem já tem 4 rodando — e a máquina travaria por uma conta que
//! estava certa isoladamente e errada no conjunto.
//!
//! ## Os três tetos aparecem, e não só o menor
//!
//! O resultado é `min(cpu, ram, load)`. Mostrar só o mínimo esconde **qual recurso está
//! apertando** — e é exatamente isso que a pessoa precisa saber para decidir se fecha um
//! navegador (RAM) ou espera um build terminar (load).

use optimizer::agentes;

/// **O quê:** o orçamento de concorrência, humano ou de máquina.
///
/// **Onde:** `main`.
///
/// **Uma medição só, dois caminhos de saída** — medir duas vezes daria dois números diferentes
/// (o load muda entre uma e outra), e a tela discordaria do terminal.
pub(crate) fn agentes_cmd(json: bool) -> Result<(), String> {
    let b = agentes::budget();

    if json {
        println!("{}", super::saidajson::agentes(&b));
        return Ok(());
    }

    println!("\x1b[1mORÇAMENTO DE CONCORRÊNCIA\x1b[0m (máquina inteira)");
    println!("  threads lógicos      {}", b.snap.threads);
    println!("  RAM disponível       {} MB", b.snap.mem_available_mb);
    println!("  load (1 min)         {:.2}", b.snap.load1);
    println!("  `claude` rodando     {}", b.snap.running_claudes);
    println!();
    // Os três tetos, com o que APERTA marcado: é o que diz se vale fechar o navegador ou
    // esperar o build acabar.
    let menor = b.total_cap;
    let marca = |v: usize| if v == menor { "  \x1b[33m← aperta\x1b[0m" } else { "" };
    println!("  teto por CPU         {}{}", b.cpu_cap, marca(b.cpu_cap));
    println!("  teto por RAM         {}{}", b.ram_cap, marca(b.ram_cap));
    println!("  teto por load        {}{}", b.load_cap, marca(b.load_cap));
    println!();
    println!("  \x1b[1mteto seguro          {}\x1b[0m", b.total_cap);
    println!("  disponível agora     {}", b.available);

    if b.ram_tight {
        println!(
            "\n\x1b[33mA RAM crua não cobre nem o piso.\x1b[0m O teto abaixo é o mínimo, e usá-lo \
             pode levar a máquina ao swap."
        );
    }
    if b.available == 0 {
        // Zero não é erro: é a resposta. E ela precisa dizer o que fazer, não só o número.
        println!(
            "\n\x1b[33mA máquina está no teto.\x1b[0m Feche um `claude` antes de abrir outro, \
             ou espere o load cair."
        );
    }
    Ok(())
}
