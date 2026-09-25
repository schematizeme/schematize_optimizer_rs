//! O `--json` do Optimizer — o CONTRATO que a janela consome.
//!
//! **O quê:** a mesma informação de `diag`, `limits` e `services`, em JSON de chaves estáveis:
//! o retrato da máquina, o veredito sobre a memória compartilhada, os tetos sugeridos e os
//! serviços de boot com o custo de cada um.
//!
//! **Onde:** [`crate::cli::diag::diag_cmd`], [`crate::cli::caixas::caixas_cmd`] e
//! [`crate::cli::servicos::servicos_cmd`] quando `--json`; e a janela do optimizer, que
//! desenha a partir disto.
//!
//! ## Por que a janela NÃO pode ler a saída humana
//!
//! Já foi tentado num app irmão, e quebrou: a janela do gestor casava rótulos **em português**,
//! então lia certo num idioma e devolvia tudo vazio nos outros dezenove — **sem erro nenhum**.
//! Com os campos vazios ela afirmava "app não instalado" a quem tinha o app. Parsear saída
//! feita para humano é contrato de mentira: passa no teste de quem escreveu e falha na máquina
//! de quem usa.
//!
//! ## A regra deste arquivo: NENHUMA prosa entra no JSON
//!
//! O relatório humano do optimizer passa inteiro pelo catálogo i18n (ADR-0014 D8). Aqui, nada
//! passa: os documentos são **byte a byte idênticos em qualquer idioma**, e existe um teste que
//! roda os três com `LANG` diferente e compara.
//!
//! O preço é que um motivo como "no Intel a memória da iGPU é fixada pelo firmware" não viaja
//! como frase, e sim como **chave de catálogo** (`"reason": "gpu.intel_firmware"`). O conjunto
//! de motivos é FECHADO — são quatro —, então a janela tem uma tabela de quatro entradas e
//! renderiza no idioma dela. Isso é o certo por outro motivo também: quem desenha é a janela,
//! e o D4 manda que a casca não saiba nada do domínio **além do que o `--json` diz**.
//!
//! Uma frase pronta pareceria mais cômodo e traria de volta exatamente o acoplamento que o
//! bug ensinou a evitar: a janela passaria a depender da prosa da CLI, e mudar uma vírgula no
//! catálogo mudaria o contrato sem aparecer em diff nenhum.
//!
//! **JSON escrito à mão, não `serde::Serialize`:** o shape é o contrato, e escrevê-lo
//! explicitamente faz uma mudança nele aparecer no diff. Com `Serialize`, renomear um campo
//! mudaria o JSON em silêncio — e quem quebraria seria a janela de quem já atualizou.

use optimizer::caixas::slice::Caixa;
use optimizer::diag::gpu::{Gpu, Limitavel, Vendor};
use optimizer::diag::maquina::Maquina;
use optimizer::servicos::Servico;

/// **O quê:** escapa o que vai dentro de aspas em JSON. **Onde:** todo valor de string daqui.
///
/// Barra invertida ANTES da aspa: na ordem inversa a barra que escapa a aspa seria ela mesma
/// escapada, e o documento sairia com uma barra a mais.
fn esc(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// **O quê:** `Some(v)` vira `"v"` escapado; `None` vira `null` — nunca `""`.
/// **Onde:** todo campo opcional de string daqui.
///
/// A janela precisa distinguir "não sei" de "vazio": foi confundir os dois que fez a janela
/// irmã afirmar "app não instalado" a quem tinha o app.
fn opt_s(v: Option<&str>) -> String {
    v.map(|v| format!("\"{}\"", esc(v))).unwrap_or_else(|| "null".into())
}

/// **O quê:** `Some(n)` vira o número; `None` vira `null`. **Onde:** os campos numéricos que
/// podem faltar (`vram_bytes`, `gtt_bytes`, `cpu_quota_pct`, `cost_s`).
fn opt_n<T: std::fmt::Display>(v: Option<T>) -> String {
    v.map(|v| v.to_string()).unwrap_or_else(|| "null".into())
}

/// **O quê:** o slug ESTÁVEL do fabricante da GPU. **Onde:** [`diag`].
///
/// Slug próprio, e não `format!("{:?}")` do enum: o `Debug` é para quem depura, e renomear uma
/// variante mudaria o contrato sem ninguém notar. O relatório humano usa o `Debug`; a janela
/// usa isto.
fn slug_vendor(v: Vendor) -> &'static str {
    match v {
        Vendor::Amd => "amd",
        Vendor::Intel => "intel",
        Vendor::Nvidia => "nvidia",
        Vendor::Desconhecido => "unknown",
    }
}

/// **O quê:** imprime o `diag` inteiro em JSON. **Onde:** `schematize-optimizer diag --json`.
///
/// `vram_cap` é sempre um objeto com as MESMAS cinco chaves, mesmo quando não dá para limitar
/// — um shape que muda de forma conforme o caso obriga a janela a ter dois parsers, e o
/// segundo é o que ninguém testa.
pub(crate) fn diag(m: &Maquina, g: &Gpu, cgroup_v2: bool, no_menu: bool) {
    let (limitavel, param, atual, sugerido, motivo) =
        match optimizer::diag::gpu::avaliar(g, m.ram_bytes) {
            Limitavel::Sim { parametro, atual_mib, sugerido_mib } => {
                (true, Some(parametro), Some(atual_mib), Some(sugerido_mib), None)
            }
            Limitavel::Nao(chave) => (false, None, None, None, Some(chave)),
        };

    println!("{{");
    println!("  \"optimizer\": \"{}\",", env!("CARGO_PKG_VERSION"));
    println!("  \"machine\": {{");
    println!("    \"ram_bytes\": {},", m.ram_bytes);
    println!("    \"cores\": {},", m.nucleos);
    println!("    \"cgroup_v2\": {cgroup_v2}");
    println!("  }},");
    println!("  \"gpu\": {{");
    println!("    \"vendor\": \"{}\",", slug_vendor(g.vendor));
    println!("    \"vram_bytes\": {},", opt_n(g.vram_bytes));
    println!("    \"gtt_bytes\": {}", opt_n(g.gtt_bytes));
    println!("  }},");
    println!("  \"vram_cap\": {{");
    println!("    \"limitable\": {limitavel},");
    println!("    \"param\": {},", opt_s(param));
    println!("    \"current_mib\": {},", opt_n(atual));
    println!("    \"suggested_mib\": {},", opt_n(sugerido));
    // CHAVE de catálogo, nunca a frase: ver o cabeçalho do módulo.
    println!("    \"reason\": {}", opt_s(motivo.as_deref()));
    println!("  }},");
    println!("  \"in_menu\": {}", !no_menu);
    println!("}}");
}

/// **O quê:** uma caixa em JSON. **Onde:** [`limits`].
fn linha_caixa(c: &Caixa) -> String {
    format!(
        "    {{\"name\": \"{}\", \"description\": \"{}\", \"memory_high_mib\": {}, \
         \"memory_max_mib\": {}, \"cpu_quota_pct\": {}, \"io_weight\": {}}}",
        esc(&c.nome),
        // `descricao` JÁ é chave de catálogo no domínio — viaja como está, sem traduzir.
        esc(&c.descricao),
        c.memory_high_mib,
        c.memory_max_mib,
        opt_n(c.cpu_quota_pct),
        opt_n(c.io_weight),
    )
}

/// **O quê:** imprime os tetos em JSON. **Onde:** `schematize-optimizer limits --json`.
///
/// `applied` diz quais slices ESTÃO no disco agora, separado de `boxes`, que é o que seria
/// sugerido. A tela precisa dos dois para mostrar o que MUDA na máquina antes de aplicar — é
/// onde a janela ganha da CLI, e sem os dois lados não há diff a mostrar.
pub(crate) fn limits(caixas: &[Caixa], cgroup_v2: bool, aplicados: &[String]) {
    println!("{{");
    println!("  \"optimizer\": \"{}\",", env!("CARGO_PKG_VERSION"));
    println!("  \"cgroup_v2\": {cgroup_v2},");
    println!("  \"boxes\": [");
    println!("{}", caixas.iter().map(linha_caixa).collect::<Vec<_>>().join(",\n"));
    println!("  ],");
    println!(
        "  \"applied\": [{}]",
        aplicados.iter().map(|n| format!("\"{}\"", esc(n))).collect::<Vec<_>>().join(", ")
    );
    println!("}}");
}

/// **O quê:** um serviço em JSON. **Onde:** [`services`].
///
/// Os três campos de justificativa são CHAVES de catálogo (é como o domínio já os guarda), e
/// `null` quando o serviço não está na allowlist — distinto de uma allowlist com texto vazio.
///
/// ## `on_allowlist` e `recommended` são DOIS campos, e o teste é quem provou que precisam ser
///
/// A primeira versão disto tinha um campo só, `safe_to_disable`, alimentado por
/// [`optimizer::servicos::vale_a_pena`]. O teste de shape reprovou na hora, com um caso real
/// desta máquina: `appstream-sync-cache.service` **está** na allowlist, com os três porquês
/// escritos, e mesmo assim `vale_a_pena` dava `false` — porque ele não apareceu no
/// `systemd-analyze blame`, ou seja, **não custa nada aqui**.
///
/// As duas afirmações são diferentes e a tela precisa das duas: "é seguro desligar" é uma
/// propriedade do serviço, auditada e justificada; "vale a pena desligar **nesta máquina**" é
/// uma medição. Achatá-las num campo só faria a tela esconder a justificativa de um item
/// seguro só porque ele é barato — e o módulo de serviços diz o contrário: se o serviço não
/// custa nada aqui, a ferramenta diz isso e **não insiste**.
fn linha_servico(s: &Servico) -> String {
    let (faz, por_que, perde) = match s.seguro {
        Some(seg) => (Some(seg.o_que_faz), Some(seg.por_que_da), Some(seg.o_que_se_perde)),
        None => (None, None, None),
    };
    format!(
        "    {{\"unit\": \"{}\", \"cost_s\": {}, \"on_allowlist\": {}, \
         \"recommended\": {}, \"does\": {}, \"why_safe\": {}, \"what_you_lose\": {}}}",
        esc(&s.unidade),
        // Duas casas fixas: `1.0` e `1` são o mesmo número, mas não o mesmo documento, e o
        // teste de byte-identidade compara documento.
        s.custo_s.map(|c| format!("{c:.2}")).unwrap_or_else(|| "null".into()),
        s.seguro.is_some(),
        optimizer::servicos::vale_a_pena(s),
        opt_s(faz),
        opt_s(por_que),
        opt_s(perde),
    )
}

/// **O quê:** imprime os serviços de boot em JSON.
/// **Onde:** `schematize-optimizer services --json`.
///
/// `systemd` distingue "esta máquina não tem systemd" de "não há serviço habilitado". O
/// relatório humano trata o primeiro como erro; aqui ele é um campo, porque uma janela que
/// recebesse erro não teria o que desenhar e mostraria tela vazia sem dizer por quê.
pub(crate) fn services(lista: &[Servico], tem_systemd: bool) {
    println!("{{");
    println!("  \"optimizer\": \"{}\",", env!("CARGO_PKG_VERSION"));
    println!("  \"systemd\": {tem_systemd},");
    println!("  \"services\": [");
    println!("{}", lista.iter().map(linha_servico).collect::<Vec<_>>().join(",\n"));
    println!("  ]");
    println!("}}");
}

// ---------------------------------------------------------------------------
// DISCO e AGENTES — chegaram na fase E3 do ADR-0018, vindos do app principal.
// ---------------------------------------------------------------------------

/// **O quê:** o slug ESTÁVEL de um tipo de achado de disco. **Onde:** [`disco`].
///
/// **Slug próprio, e não o `rotulo()`:** aquele é PROSA para humano (`"target (Rust)"`, com
/// espaço e parêntese) e muda de redação; este é o que a janela casa para escolher o ícone e o
/// filtro. Misturar os dois é o defeito que este projeto já pagou duas vezes.
fn slug_tipo(t: optimizer::disco::Tipo) -> &'static str {
    use optimizer::disco::Tipo;
    match t {
        Tipo::RustTarget => "rust-target",
        Tipo::NodeModules => "node-modules",
        Tipo::NodeBuild => "node-build",
        Tipo::NodeCache => "npm-cache",
        Tipo::GoCache => "go-cache",
        Tipo::PythonCache => "python-cache",
        Tipo::PythonVenv => "python-venv",
        Tipo::CargoCache => "cargo-cache",
    }
}

/// **O quê:** o inventário de disco como documento de máquina.
///
/// **Onde:** `disco --json`, que é o que a aba Disco da janela lê.
///
/// **Os TRÊS cortes vão juntos** (por disco, por tipo, e os achados) porque são a mesma varredura
/// vista de três ângulos. Fazer a janela recalcular os agregados a partir da lista significaria
/// duas somas do mesmo número, e a que divergisse mostraria um total que não bate com as linhas.
///
/// **`bytes` cru, sem formatar.** `"1.2 GB"` é prosa: depende de idioma (vírgula ou ponto) e de
/// arredondamento. A janela formata; o contrato carrega o número.
///
/// **`refaz_text` é o ÚNICO campo de prosa, e o `_text` no nome é a declaração disso** — é o
/// precedente que o `status_text` do market abriu. Ele explica em uma frase como o artefato se
/// refaz (*"recriado no próximo go build"*), e não há como reduzir isso a um slug sem perder a
/// informação. Quem lê sabe, pelo nome, que não pode ramificar por ele. O campo que a janela
/// ramifica é o `tipo`, que é slug.
pub(crate) fn disco(
    achados: &[optimizer::disco::Achado],
    docker: &[optimizer::disco::docker::Categoria],
) -> String {
    let por_montagem: Vec<String> = optimizer::disco::por_montagem(achados)
        .into_iter()
        .map(|(m, b)| format!("{{\"montagem\":\"{}\",\"bytes\":{}}}", esc(&m.to_string_lossy()), b))
        .collect();
    let por_tipo: Vec<String> = optimizer::disco::por_tipo(achados)
        .into_iter()
        .map(|(t, b)| {
            format!(
                "{{\"tipo\":\"{}\",\"bytes\":{},\"custa_rede\":{}}}",
                slug_tipo(t),
                b,
                t.custa_rede()
            )
        })
        .collect();
    let itens: Vec<String> = achados
        .iter()
        .map(|a| {
            format!(
                "{{\"caminho\":\"{}\",\"tipo\":\"{}\",\"bytes\":{},\"dias_parado\":{},\
                 \"montagem\":\"{}\",\"refaz_text\":\"{}\"}}",
                esc(&a.caminho.to_string_lossy()),
                slug_tipo(a.tipo),
                a.bytes,
                a.dias_parado,
                esc(&a.montagem.to_string_lossy()),
                esc(a.refaz)
            )
        })
        .collect();
    let cats: Vec<String> = docker
        .iter()
        .map(|c| {
            format!(
                "{{\"tipo\":\"{}\",\"bytes\":{},\"recuperavel\":{}}}",
                esc(&c.tipo),
                c.bytes,
                c.recuperavel
            )
        })
        .collect();
    // `docker_disponivel` separado da lista vazia: máquina SEM docker e máquina com docker e
    // nada a recuperar são estados diferentes, e a janela desenha os dois de formas diferentes.
    // `optimizer` em todo documento, como nos três irmãos: quem lê precisa saber de QUE versão
    // do app o contrato veio, senão um campo ausente é indistinguível de um app antigo.
    format!(
        "{{\"optimizer\":\"{}\",\"total_bytes\":{},\"por_montagem\":[{}],\"por_tipo\":[{}],\
         \"achados\":[{}],\"docker_disponivel\":{},\"docker\":[{}]}}",
        env!("CARGO_PKG_VERSION"),
        achados.iter().map(|a| a.bytes).sum::<u64>(),
        por_montagem.join(","),
        por_tipo.join(","),
        itens.join(","),
        optimizer::disco::docker::disponivel(),
        cats.join(",")
    )
}

/// **O quê:** o orçamento de concorrência como documento de máquina.
///
/// **Onde:** `agentes --json`.
///
/// **Os três tetos entram, e não só o menor:** o `total_cap` é o mínimo deles, e quem lê precisa
/// saber QUAL recurso está apertando para decidir o que fazer. Um documento com só o resultado
/// obrigaria a janela a adivinhar — ou a recalcular, que é duas leis para o mesmo número.
pub(crate) fn agentes(b: &optimizer::agentes::Budget) -> String {
    format!(
        "{{\"optimizer\":\"{}\",\"threads\":{},\"mem_available_mb\":{},\"load1\":{},\
         \"running_claudes\":{},\"cpu_cap\":{},\"ram_cap\":{},\"load_cap\":{},\
         \"total_cap\":{},\"available\":{},\"ram_tight\":{}}}",
        env!("CARGO_PKG_VERSION"),
        b.snap.threads,
        b.snap.mem_available_mb,
        // O load é fracionário e vai como NÚMERO, com duas casas: mandá-lo como string faria a
        // janela parsear texto, e `0,75` contra `0.75` depende do idioma de quem formatou.
        format_args!("{:.2}", b.snap.load1),
        b.snap.running_claudes,
        b.cpu_cap,
        b.ram_cap,
        b.load_cap,
        b.total_cap,
        b.available,
        b.ram_tight
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Aspas e barras dentro de um valor não podem quebrar o JSON. Um nome de unidade ou um
    /// caminho com aspas viraria documento inválido, e a janela mostraria tela vazia sem
    /// dizer por quê.
    #[test]
    fn escapa_aspas_e_barras() {
        assert_eq!(esc(r#"a"b"#), r#"a\"b"#);
        assert_eq!(esc(r"a\b"), r"a\\b");
        // A barra é escapada ANTES da aspa; na ordem inversa a barra da aspa seria
        // re-escapada e o resultado teria uma barra a mais.
        assert_eq!(esc(r#"\""#), r#"\\\""#);
    }

    /// `None` vira `null`, NUNCA `""` nem `0`. A janela precisa separar "não sei" de "vazio"
    /// e de "zero" — foi confundir os dois primeiros que fez a janela irmã afirmar "app não
    /// instalado" a quem tinha o app.
    #[test]
    fn ausente_e_null_e_nao_vazio_nem_zero() {
        assert_eq!(opt_s(None), "null");
        assert_eq!(opt_s(Some("")), r#""""#);
        assert_ne!(opt_s(None), opt_s(Some("")));

        assert_eq!(opt_n::<u64>(None), "null");
        assert_eq!(opt_n(Some(0u64)), "0");
        assert_ne!(opt_n::<u64>(None), opt_n(Some(0u64)));
    }

    /// **O slug do vendor é CONTRATO, e por isso não é o `Debug` do enum.** Renomear uma
    /// variante mudaria o `Debug` — e mudaria o JSON — sem aparecer em lugar nenhum.
    #[test]
    fn slug_do_vendor_e_estavel_e_minusculo() {
        assert_eq!(slug_vendor(Vendor::Amd), "amd");
        assert_eq!(slug_vendor(Vendor::Intel), "intel");
        assert_eq!(slug_vendor(Vendor::Nvidia), "nvidia");
        assert_eq!(slug_vendor(Vendor::Desconhecido), "unknown");
        // O `Debug` é outra coisa, de propósito: ele é para quem depura.
        assert_ne!(slug_vendor(Vendor::Amd), format!("{:?}", Vendor::Amd));
    }

    /// **O contrato do `limits`.** Estas chaves são lidas pela janela; renomear qualquer uma
    /// quebra a janela de quem já atualizou, e o JSON à mão existe para que isso apareça no
    /// diff.
    #[test]
    fn as_chaves_do_contrato_de_limits_estao_todas_la() {
        let c = &optimizer::caixas::slice::sugerir(16384, 8)[0];
        let j = linha_caixa(c);
        for k in [
            "name",
            "description",
            "memory_high_mib",
            "memory_max_mib",
            "cpu_quota_pct",
            "io_weight",
        ] {
            assert!(j.contains(&format!("\"{k}\"")), "faltou a chave `{k}` em: {j}");
        }
    }

    /// **O contrato do `services`**, e o `null` de quem não está na allowlist — que é
    /// diferente de estar nela com texto vazio.
    #[test]
    fn as_chaves_do_contrato_de_services_e_o_null_de_fora_da_allowlist() {
        let fora =
            Servico { unidade: "firewalld.service".into(), custo_s: Some(0.5), seguro: None };
        let j = linha_servico(&fora);
        for k in
            ["unit", "cost_s", "on_allowlist", "recommended", "does", "why_safe", "what_you_lose"]
        {
            assert!(j.contains(&format!("\"{k}\"")), "faltou `{k}`: {j}");
        }
        assert!(j.contains("\"does\": null"), "fora da allowlist é null, não vazio: {j}");
        assert!(j.contains("\"on_allowlist\": false"), "{j}");

        let dentro = Servico {
            unidade: "NetworkManager-wait-online.service".into(),
            custo_s: Some(12.0),
            seguro: optimizer::servicos::seguro_de("NetworkManager-wait-online.service"),
        };
        let j = linha_servico(&dentro);
        assert!(j.contains("\"on_allowlist\": true"), "{j}");
        assert!(j.contains("\"recommended\": true"), "custa 12s: vale a pena aqui — {j}");
        // CHAVE de catálogo, nunca a frase traduzida — é o que mantém o documento
        // byte-idêntico entre idiomas.
        assert!(j.contains("\"does\": \"svc.nm_wait.does\""), "{j}");
        assert!(!j.contains("boot"), "prosa traduzida não entra no contrato: {j}");
    }

    /// **Seguro e caro são coisas DIFERENTES, e o JSON não as achata.** Este é o caso real
    /// que reprovou a primeira versão do contrato: um serviço da allowlist que não apareceu no
    /// `blame` desta máquina. Ele continua sendo seguro desligar — só não vale a pena aqui, e
    /// a tela tem de poder dizer as duas coisas.
    #[test]
    fn da_allowlist_mas_sem_custo_medido_e_seguro_sem_ser_recomendado() {
        let s = Servico {
            unidade: "appstream-sync-cache.service".into(),
            custo_s: None,
            seguro: optimizer::servicos::seguro_de("appstream-sync-cache.service"),
        };
        let j = linha_servico(&s);
        assert!(j.contains("\"on_allowlist\": true"), "está na allowlist, e auditado: {j}");
        assert!(j.contains("\"recommended\": false"), "não custa nada aqui: {j}");
        // E a justificativa continua lá: escondê-la só porque é barato tiraria da pessoa o
        // que ela precisa para decidir.
        assert!(j.contains("\"does\": \"svc.appstream.does\""), "{j}");
    }

    /// O custo sai com DUAS casas sempre. `1.0` e `1` são o mesmo número e documentos
    /// diferentes, e o teste de byte-identidade compara documento.
    #[test]
    fn custo_tem_casas_fixas_e_ausencia_e_null() {
        let s = Servico { unidade: "a.service".into(), custo_s: Some(1.0), seguro: None };
        assert!(linha_servico(&s).contains("\"cost_s\": 1.00"), "{}", linha_servico(&s));
        let s = Servico { unidade: "a.service".into(), custo_s: None, seguro: None };
        assert!(linha_servico(&s).contains("\"cost_s\": null"), "{}", linha_servico(&s));
    }
}
