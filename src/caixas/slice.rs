//! CAIXAS — pôr build, navegador e container cada um no seu teto, via systemd.
//!
//! **O quê:** gera *slices* de usuário (`~/.config/systemd/user/*.slice`) com limites de
//! memória, CPU e IO, e sabe desfazê-los.
//!
//! **Onde:** `optimizer limits`. É a única área que o `--aplicar` executa sozinho (ADR-0011),
//! e a razão é o custo do erro: slice é reversível apagando um arquivo, não pede root nem
//! reboot, e no pior caso um build fica mais lento.
//!
//! ## Por que systemd slice, e não `ulimit` nem `nice`
//!
//! `ulimit` é por processo e não segura uma árvore — um `cargo build` que dispara 16 `rustc`
//! escapa. `nice` mexe em prioridade, não em teto: o build ainda toma a RAM toda, só mais
//! educadamente. **Slice é cgroup v2**, e cgroup conta a árvore inteira: o limite vale para o
//! processo e para tudo que ele criar.
//!
//! ## `MemoryHigh` antes de `MemoryMax` — e a diferença importa
//!
//! `MemoryMax` é a parede: estourou, o kernel **mata** (OOM). `MemoryHigh` é a pressão: ao
//! passar, o kernel começa a recuperar memória e o processo **fica lento em vez de morrer**.
//!
//! Um build morto por OOM perde uma hora de compilação e a pessoa não sabe por quê. Um build
//! lento termina. Então `MemoryHigh` é o limite de trabalho, e `MemoryMax` fica bem acima —
//! como rede contra fuga de memória, não como o teto do dia a dia.

use std::path::{Path, PathBuf};

/// Um perfil de caixa: o que limitar e quanto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Caixa {
    /// Nome do slice, sem a extensão (ex.: `dev-build`).
    pub nome: String,
    /// Para que serve — vai no arquivo, para quem abrir daqui a um ano entender.
    /// CHAVE de catálogo (não prosa): a unit escrita em disco e o relatório traduzem na
    /// hora. Antes era o texto em português, e o relatório em inglês saía com a descrição em
    /// português — ver o ADR-0014 (D8).
    pub descricao: String,
    /// Onde o kernel começa a apertar (MiB). É o teto de trabalho.
    pub memory_high_mib: u64,
    /// A parede (MiB). Rede contra fuga de memória, não limite do dia a dia.
    pub memory_max_mib: u64,
    /// Percentual de CPU: 100 = um núcleo. `None` = sem teto.
    pub cpu_quota_pct: Option<u32>,
    /// Peso de IO (1..10000, padrão 100). Menor = cede a vez a quem tem mais.
    pub io_weight: Option<u32>,
}

/// **O quê:** os perfis sugeridos para uma máquina com `ram_mib` de RAM e `nucleos` de CPU.
///
/// **Onde:** `optimizer limits`. Função PURA — a regra é testável sem systemd.
///
/// **A regra, e o porquê de cada número:**
///
/// - **build**: metade da RAM em `High`. Compilação é o que mais infla, e é o que a pessoa
///   *quer* que use a máquina — só não a ponto de levar o resto junto. CPU sem teto: um build
///   lento por teto de CPU é o oposto do pedido.
/// - **navegador**: um quarto da RAM. Ele cresce sem parar e ninguém percebe, e é o suspeito
///   nº 1 de "a máquina travou enquanto eu compilava".
/// - **container**: um quarto. Docker/Podman sem teto é a outra metade do mesmo problema.
///
/// Nenhum deles ganha `MemoryMax` colado no `High`: a diferença é o que troca "morreu" por
/// "ficou lento".
pub fn sugerir(ram_mib: u64, nucleos: u32) -> Vec<Caixa> {
    let metade = (ram_mib / 2).max(2048);
    let quarto = (ram_mib / 4).max(1024);
    vec![
        Caixa {
            nome: "dev-build".into(),
            descricao: "box.build".into(),
            memory_high_mib: metade,
            memory_max_mib: (metade * 3 / 2).min(ram_mib * 9 / 10),
            // Sem teto de CPU de propósito: build lento por cota é o oposto do que se pediu.
            // O que se quer é que ele não coma a RAM toda, não que compile devagar.
            cpu_quota_pct: None,
            io_weight: Some(50), // cede a vez ao interativo: o desktop continua responsivo
        },
        Caixa {
            nome: "dev-navegador".into(),
            descricao: "box.browser".into(),
            memory_high_mib: quarto,
            memory_max_mib: quarto * 3 / 2,
            // Um núcleo e meio: o suficiente para o navegador não engasgar, longe de competir
            // com um build de 12 threads.
            cpu_quota_pct: Some((nucleos.min(2) * 100).max(150)),
            io_weight: Some(100),
        },
        Caixa {
            nome: "dev-container".into(),
            descricao: "box.container".into(),
            memory_high_mib: quarto,
            memory_max_mib: quarto * 3 / 2,
            cpu_quota_pct: Some(nucleos.saturating_mul(100).saturating_sub(100).max(100)),
            io_weight: Some(80),
        },
    ]
}

/// **O quê:** o conteúdo do arquivo `.slice`, exatamente como vai para o disco.
///
/// **Onde:** [`aplicar`] e o teste. Função PURA — é o que permite afirmar o formato sem
/// escrever em `~/.config`.
///
/// O cabeçalho diz **quem gerou e como desfazer**. Um arquivo de systemd sem isso vira
/// mistério em três meses, e mistério em `~/.config/systemd` é o que faz alguém apagar a
/// pasta inteira por precaução.
pub fn render(c: &Caixa) -> String {
    let mut s = String::new();
    s.push_str("# Gerado por `schematize optimizer` (ADR-0011). NÃO edite à mão:\n");
    s.push_str(
        "# `optimizer limits --revert` apaga este arquivo e devolve o sistema ao que era.\n",
    );
    // A descrição é CHAVE de catálogo; aqui ela vira texto. O arquivo de unit é lido por
    // humano (e mostrado pelo `systemctl status`), então gravar a chave crua poria
    // `box.build` na cara de quem for investigar.
    let descricao = crate::nucleo::i18n::t(&c.descricao);
    s.push_str(&format!("# O quê: {descricao}\n"));
    s.push_str("#\n");
    s.push_str("# MemoryHigh é onde o kernel começa a APERTAR (fica lento);\n");
    s.push_str("# MemoryMax é onde ele MATA. A distância entre os dois é o que troca\n");
    s.push_str("# \"o build morreu depois de uma hora\" por \"o build demorou mais\".\n");
    s.push_str("[Unit]\n");
    s.push_str(&format!("Description={descricao}\n\n"));
    s.push_str("[Slice]\n");
    s.push_str(&format!("MemoryHigh={}M\n", c.memory_high_mib));
    s.push_str(&format!("MemoryMax={}M\n", c.memory_max_mib));
    if let Some(q) = c.cpu_quota_pct {
        s.push_str(&format!("CPUQuota={q}%\n"));
    }
    if let Some(w) = c.io_weight {
        s.push_str(&format!("IOWeight={w}\n"));
    }
    s
}

/// **O quê:** onde os slices de usuário moram.
/// **Onde:** [`aplicar`] e [`reverter`].
pub fn dir_slices(home: &Path) -> PathBuf {
    home.join(".config/systemd/user")
}

/// **O quê:** grava os slices. Devolve os caminhos escritos.
///
/// **Onde:** `optimizer limits --apply`.
///
/// **Só escreve arquivo que ele mesmo gerou:** se já existe um `.slice` de mesmo nome SEM o
/// cabeçalho desta ferramenta, ele é **preservado** e reportado. Sobrescrever configuração
/// que outra pessoa (ou outra ferramenta) pôs ali é o erro que não se desfaz com `--revert`,
/// porque o `--revert` não sabe o que havia antes.
pub fn aplicar(home: &Path, caixas: &[Caixa]) -> Result<Vec<PathBuf>, String> {
    let dir = dir_slices(home);
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("não consegui criar {}: {e}", dir.display()))?;
    let mut escritos = Vec::new();
    for c in caixas {
        let p = dir.join(format!("{}.slice", c.nome));
        if let Ok(atual) = std::fs::read_to_string(&p) {
            if !nosso(&atual) {
                return Err(format!(
                    "{} já existe e NÃO foi gerado por esta ferramenta — não vou sobrescrever \
                     configuração que não sei desfazer. Mova ou apague o arquivo e repita",
                    p.display()
                ));
            }
        }
        std::fs::write(&p, render(c))
            .map_err(|e| format!("não consegui escrever {}: {e}", p.display()))?;
        escritos.push(p);
    }
    Ok(escritos)
}

/// **O quê:** este arquivo foi gerado por nós?
///
/// **Onde:** [`aplicar`] e [`reverter`]. É a marca que separa "posso mexer" de "é de outro".
pub fn nosso(conteudo: &str) -> bool {
    conteudo.contains("Gerado por `schematize optimizer`")
}

/// **O quê:** apaga os slices que ESTA ferramenta gerou, e só eles. Devolve o que apagou.
///
/// **Onde:** `optimizer limits --revert`.
///
/// **Nunca apaga o que não reconhece.** Um `.slice` alheio com nome parecido fica onde está —
/// e é por isso que [`aplicar`] se recusa a sobrescrever: as duas metades da mesma regra.
pub fn reverter(home: &Path) -> Result<Vec<PathBuf>, String> {
    let dir = dir_slices(home);
    let mut apagados = Vec::new();
    let entradas = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        // Nunca aplicado é estado NORMAL, não erro: reverter o que não existe é sucesso vazio.
        Err(_) => return Ok(apagados),
    };
    for e in entradas.flatten() {
        let p = e.path();
        if p.extension().and_then(|x| x.to_str()) != Some("slice") {
            continue;
        }
        match std::fs::read_to_string(&p) {
            Ok(c) if nosso(&c) => {
                std::fs::remove_file(&p)
                    .map_err(|er| format!("não consegui apagar {}: {er}", p.display()))?;
                apagados.push(p);
            }
            // Ilegível ou de terceiro: fica. Não é nosso, não se mexe.
            _ => {}
        }
    }
    Ok(apagados)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sandbox(nome: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("opt-caixa-{nome}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// **`MemoryHigh` SEMPRE abaixo de `MemoryMax`.** É a distância entre os dois que troca
    /// "o build morreu depois de uma hora" por "o build demorou mais" — se colarem, a caixa
    /// deixa de proteger e passa a matar.
    #[test]
    fn high_sempre_abaixo_de_max() {
        for ram in [4096u64, 8192, 16384, 65536] {
            for c in sugerir(ram, 8) {
                assert!(
                    c.memory_high_mib < c.memory_max_mib,
                    "{}: High {} não é menor que Max {} — vira OOM em vez de lentidão",
                    c.nome,
                    c.memory_high_mib,
                    c.memory_max_mib
                );
            }
        }
    }

    /// Nenhuma caixa pode passar da RAM da máquina — teto acima do físico não é teto.
    #[test]
    fn nenhuma_caixa_passa_da_ram() {
        let ram = 16384;
        for c in sugerir(ram, 8) {
            assert!(c.memory_max_mib <= ram, "{}: Max {} > RAM {}", c.nome, c.memory_max_mib, ram);
        }
    }

    /// O build NÃO ganha teto de CPU: build lento por cota é o oposto do que se pediu. O que
    /// se quer é que ele não coma a RAM toda, não que compile devagar.
    #[test]
    fn o_build_nao_ganha_teto_de_cpu() {
        let cs = sugerir(16384, 12);
        let b = cs.iter().find(|c| c.nome == "dev-build").unwrap();
        assert!(b.cpu_quota_pct.is_none(), "teto de CPU no build contraria o objetivo");
        assert!(b.io_weight.unwrap() < 100, "o build deve CEDER IO ao interativo");
    }

    /// Máquina pequena não recebe teto que a inviabiliza — os pisos existem para isso.
    #[test]
    fn maquina_pequena_ainda_recebe_teto_utilizavel() {
        for c in sugerir(2048, 2) {
            assert!(c.memory_high_mib >= 1024, "{}: teto pequeno demais para servir", c.nome);
        }
    }

    /// O arquivo gerado diz quem o criou e como desfazer — sem isso vira mistério em
    /// `~/.config/systemd`, e mistério ali faz alguém apagar a pasta inteira.
    #[test]
    fn o_arquivo_se_explica_e_diz_como_desfazer() {
        let c = &sugerir(16384, 8)[0];
        let t = render(c);
        assert!(t.contains("[Slice]") && t.contains("MemoryHigh="));
        assert!(t.contains("--revert"), "tem de dizer como desfazer: {t}");
        assert!(nosso(&t), "tem de carregar a marca que permite reverter");
    }

    /// Ida e volta: aplicar e reverter deixa o diretório como estava.
    #[test]
    fn aplicar_e_reverter_volta_ao_estado_anterior() {
        let h = sandbox("roundtrip");
        let cs = sugerir(16384, 8);
        let escritos = aplicar(&h, &cs).unwrap();
        assert_eq!(escritos.len(), cs.len());
        for p in &escritos {
            assert!(p.exists());
        }
        let apagados = reverter(&h).unwrap();
        assert_eq!(apagados.len(), cs.len());
        for p in &escritos {
            assert!(!p.exists(), "sobrou {}", p.display());
        }
        let _ = std::fs::remove_dir_all(&h);
    }

    /// **A regra que impede o dano irreversível:** um `.slice` que NÃO é nosso não é
    /// sobrescrito. O `--revert` não saberia o que havia antes, então sobrescrever seria uma
    /// mudança sem desfazer — o que o ADR-0011 proíbe.
    #[test]
    fn nao_sobrescreve_slice_de_terceiro() {
        let h = sandbox("alheio");
        let dir = dir_slices(&h);
        std::fs::create_dir_all(&dir).unwrap();
        let alheio = dir.join("dev-build.slice");
        std::fs::write(&alheio, "[Slice]\nMemoryMax=1G\n# meu, feito a mao\n").unwrap();

        let e = aplicar(&h, &sugerir(16384, 8)).unwrap_err();
        assert!(e.contains("não vou sobrescrever"), "{e}");
        // E o arquivo continua exatamente como estava.
        let depois = std::fs::read_to_string(&alheio).unwrap();
        assert!(depois.contains("feito a mao"), "o arquivo alheio foi alterado");
        let _ = std::fs::remove_dir_all(&h);
    }

    /// E a outra metade da mesma regra: o `--revert` também não apaga o que não é nosso.
    #[test]
    fn revert_nao_apaga_slice_de_terceiro() {
        let h = sandbox("revertalheio");
        let dir = dir_slices(&h);
        std::fs::create_dir_all(&dir).unwrap();
        let alheio = dir.join("outra-coisa.slice");
        std::fs::write(&alheio, "[Slice]\nMemoryMax=1G\n").unwrap();
        std::fs::write(dir.join("dev-build.slice"), render(&sugerir(16384, 8)[0])).unwrap();

        let apagados = reverter(&h).unwrap();
        assert_eq!(apagados.len(), 1, "só o nosso podia sair");
        assert!(alheio.exists(), "apagou arquivo de terceiro");
        let _ = std::fs::remove_dir_all(&h);
    }

    /// Reverter sem nunca ter aplicado é sucesso vazio, não erro.
    #[test]
    fn reverter_sem_ter_aplicado_nao_e_erro() {
        let h = sandbox("nunca");
        assert_eq!(reverter(&h).unwrap().len(), 0);
        let _ = std::fs::remove_dir_all(&h);
    }
}
