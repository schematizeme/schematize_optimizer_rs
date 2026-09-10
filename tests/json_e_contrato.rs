//! O `--json` é CONTRATO: byte a byte igual em qualquer idioma, e sempre JSON válido.
//!
//! # Por que este teste existe
//!
//! Um app irmão já quebrou exatamente aqui. A janela do gestor lia a saída HUMANA do `status`
//! e casava os rótulos **em português**. O relatório passa pelo catálogo i18n, então em
//! qualquer um dos outros idiomas os rótulos eram outros — e o parse devolvia tudo vazio,
//! **sem erro nenhum**. Com os campos vazios a janela afirmava "app não instalado" a quem
//! tinha o app, e a pessoa clicava em "Instalar" sem que nada acontecesse.
//!
//! A lição não é "traduza melhor": é que **saída para humano não é contrato**. A saída deste
//! app é traduzida de ponta a ponta (ADR-0014 D8) e vai continuar sendo. O `--json` é a outra
//! porta, e o que a torna confiável é não ter prosa nenhuma dentro.
//!
//! # O que ele afirma
//!
//! 1. Os três documentos são **byte a byte idênticos** com `LANG` de cada idioma do catálogo.
//! 2. São JSON **válido** — provado por um parser de verdade, escrito aqui, que reprova o que
//!    o `esc`/`opt_*` deixasse passar.
//! 3. As chaves do contrato estão todas lá, nos três.
//!
//! # Por que ele roda o BINÁRIO, e não as funções
//!
//! As funções já têm teste unitário. O que só o binário prova é que nenhum `println!` humano
//! escapou para dentro do documento — um cabeçalho impresso antes do `{` não quebra função
//! nenhuma, e é justamente o que tornaria o JSON inválido na máquina de quem usa.

use std::collections::BTreeMap;
use std::process::Command;

/// **O quê:** o caminho do binário compilado, ao lado do executável de teste.
///
/// **Onde:** [`rodar`]. `CARGO_BIN_EXE_` seria mais curto, mas ele só existe para testes de
/// integração do MESMO pacote quando há `[[bin]]` — e amarrar o teste ao nome do binário, que
/// já mudou uma vez neste projeto, é o tipo de acoplamento que este arquivo combate.
fn bin() -> std::path::PathBuf {
    let mut p = std::env::current_exe().expect("o teste tem caminho");
    p.pop();
    if p.ends_with("deps") {
        p.pop();
    }
    p.join("schematize-optimizer")
}

/// **O quê:** roda um subcomando com `--json` num idioma, e devolve o stdout cru.
///
/// **Onde:** todos os testes daqui.
///
/// **O ambiente é limpo com `env_remove`**, e não só sobrescrito: o app resolve o idioma por
/// `OPTIMIZER_LANG` → `SCHEMATIZE_LANG` → `LC_ALL` → `LC_MESSAGES` → `LANG`, e uma variável
/// herdada da máquina de quem roda a suíte venceria a que o teste quer testar. Um teste que
/// muda de resultado conforme o `LANG` de quem o executa não prova nada — e o bug que este
/// arquivo trava é exatamente sobre idioma.
fn rodar(sub: &[&str], lang: &str) -> String {
    let mut c = Command::new(bin());
    for v in ["OPTIMIZER_LANG", "SCHEMATIZE_LANG", "LC_ALL", "LC_MESSAGES", "LANG", "LANGUAGE"] {
        c.env_remove(v);
    }
    let out = c
        .args(sub)
        .arg("--json")
        .env("LANG", lang)
        .output()
        .unwrap_or_else(|e| panic!("não consegui executar {}: {e}", bin().display()));
    assert!(
        out.status.success(),
        "`{}` saiu com {} — stderr: {}",
        sub.join(" "),
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).expect("o contrato é UTF-8")
}

// ---------------------------------------------------------------------------
// Um parser de JSON pequeno, só para AFIRMAR validade.
//
// Ele existe porque "parseia com `json.load`" é a prova que o checklist pede, e uma
// asserção de `contains("{")` não é essa prova: um documento com aspa mal escapada passa no
// `contains` e explode na janela. `serde_json` como dev-dependency resolveria — e traria uma
// dependência a mais neste repo para provar uma coisa só. São ~70 linhas.
// ---------------------------------------------------------------------------

/// O valor JSON, no mínimo que este contrato usa: objeto, lista, string, número, bool, null.
#[derive(Debug, Clone, PartialEq)]
enum Json {
    Obj(BTreeMap<String, Json>),
    Arr(Vec<Json>),
    Str(String),
    Num(f64),
    Bool(bool),
    Null,
}

impl Json {
    /// **O quê:** o valor de uma chave, se isto for objeto. **Onde:** as asserções de shape.
    fn get(&self, k: &str) -> Option<&Json> {
        match self {
            Json::Obj(m) => m.get(k),
            _ => None,
        }
    }
    /// **O quê:** os itens, se isto for lista. **Onde:** as asserções sobre `boxes`/`services`.
    fn arr(&self) -> &[Json] {
        match self {
            Json::Arr(v) => v,
            outro => panic!("esperava lista, veio {outro:?}"),
        }
    }
}

/// **O quê:** parseia um documento inteiro; entrada de lixo vira `Err` com a posição.
/// **Onde:** [`parse`], que é o que os testes chamam.
fn parse_valor(s: &[u8], i: &mut usize) -> Result<Json, String> {
    while *i < s.len() && s[*i].is_ascii_whitespace() {
        *i += 1;
    }
    if *i >= s.len() {
        return Err("documento termina onde devia haver valor".into());
    }
    match s[*i] {
        b'{' => {
            *i += 1;
            let mut m = BTreeMap::new();
            loop {
                while *i < s.len() && s[*i].is_ascii_whitespace() {
                    *i += 1;
                }
                if *i < s.len() && s[*i] == b'}' {
                    *i += 1;
                    return Ok(Json::Obj(m));
                }
                let Json::Str(k) = parse_valor(s, i)? else {
                    return Err(format!("chave tem de ser string (byte {i})"));
                };
                while *i < s.len() && s[*i].is_ascii_whitespace() {
                    *i += 1;
                }
                if *i >= s.len() || s[*i] != b':' {
                    return Err(format!("faltou `:` depois de `{k}` (byte {i})"));
                }
                *i += 1;
                let v = parse_valor(s, i)?;
                if m.insert(k.clone(), v).is_some() {
                    return Err(format!("chave `{k}` duplicada — a janela leria a última"));
                }
                while *i < s.len() && s[*i].is_ascii_whitespace() {
                    *i += 1;
                }
                if *i < s.len() && s[*i] == b',' {
                    *i += 1;
                }
            }
        }
        b'[' => {
            *i += 1;
            let mut v = Vec::new();
            loop {
                while *i < s.len() && s[*i].is_ascii_whitespace() {
                    *i += 1;
                }
                if *i < s.len() && s[*i] == b']' {
                    *i += 1;
                    return Ok(Json::Arr(v));
                }
                v.push(parse_valor(s, i)?);
                while *i < s.len() && s[*i].is_ascii_whitespace() {
                    *i += 1;
                }
                if *i < s.len() && s[*i] == b',' {
                    *i += 1;
                }
            }
        }
        b'"' => {
            *i += 1;
            let mut out = String::new();
            loop {
                if *i >= s.len() {
                    return Err("string sem aspa de fechamento".into());
                }
                match s[*i] {
                    b'"' => {
                        *i += 1;
                        return Ok(Json::Str(out));
                    }
                    b'\\' => {
                        *i += 1;
                        if *i >= s.len() {
                            return Err("escape no fim do documento".into());
                        }
                        out.push(s[*i] as char);
                        *i += 1;
                    }
                    c => {
                        out.push(c as char);
                        *i += 1;
                    }
                }
            }
        }
        _ => {
            let ini = *i;
            while *i < s.len() && !b",}] \n\t\r".contains(&s[*i]) {
                *i += 1;
            }
            let lit = std::str::from_utf8(&s[ini..*i]).map_err(|e| e.to_string())?;
            match lit {
                "true" => Ok(Json::Bool(true)),
                "false" => Ok(Json::Bool(false)),
                "null" => Ok(Json::Null),
                n => n.parse::<f64>().map(Json::Num).map_err(|_| format!("`{n}` não é valor JSON")),
            }
        }
    }
}

/// **O quê:** parseia o documento e exige que ele acabe ali. **Onde:** todos os testes.
///
/// A sobra no fim importa: um `println!` humano depois do `}` deixaria o documento "válido"
/// para um parser tolerante e quebraria um estrito — e a janela usa um estrito.
fn parse(txt: &str) -> Json {
    let b = txt.as_bytes();
    let mut i = 0;
    let v = parse_valor(b, &mut i).unwrap_or_else(|e| panic!("JSON inválido: {e}\n---\n{txt}"));
    let resto = txt[i..].trim();
    assert!(resto.is_empty(), "sobrou coisa depois do JSON (prosa vazada?): {resto:?}");
    v
}

/// Todos os idiomas do catálogo, mais um que não existe — o fallback também não pode mudar
/// o documento.
const IDIOMAS: &[&str] = &["en_US.UTF-8", "pt_BR.UTF-8", "C", "ja_JP.UTF-8"];

/// **O BUG QUE ESTE ARQUIVO TRAVA.** O documento é o mesmo em qualquer idioma — byte a byte.
///
/// Se um dia alguém puser uma frase traduzida num campo (`"reason": "no Intel a memória…"`),
/// este teste falha na hora, e não seis meses depois na máquina de um usuário japonês.
#[test]
fn os_tres_json_sao_byte_a_byte_iguais_em_qualquer_idioma() {
    for sub in [&["diag"][..], &["limits"][..], &["services"][..]] {
        let referencia = rodar(sub, IDIOMAS[0]);
        for lang in &IDIOMAS[1..] {
            let outro = rodar(sub, lang);
            assert_eq!(
                referencia,
                outro,
                "`{} --json` mudou entre {} e {lang} — prosa traduzida vazou para o contrato",
                sub.join(" "),
                IDIOMAS[0]
            );
        }
    }
}

/// Os três documentos são JSON válido de verdade, e acabam onde dizem acabar.
#[test]
fn os_tres_json_sao_validos_e_nao_tem_sobra() {
    for sub in [&["diag"][..], &["limits"][..], &["services"][..]] {
        let v = parse(&rodar(sub, "en_US.UTF-8"));
        assert!(v.get("optimizer").is_some(), "todo documento diz de que versão veio: {sub:?}");
    }
}

/// **O contrato do `diag`.** Estas chaves são lidas pela tela de diagnóstico; renomear
/// qualquer uma quebra a janela de quem já atualizou.
#[test]
fn o_shape_do_diag_e_contrato() {
    let v = parse(&rodar(&["diag"], "en_US.UTF-8"));
    for k in ["optimizer", "machine", "gpu", "vram_cap", "in_menu"] {
        assert!(v.get(k).is_some(), "faltou `{k}`");
    }
    let m = v.get("machine").unwrap();
    for k in ["ram_bytes", "cores", "cgroup_v2"] {
        assert!(m.get(k).is_some(), "faltou `machine.{k}`");
    }
    let g = v.get("gpu").unwrap();
    for k in ["vendor", "vram_bytes", "gtt_bytes"] {
        assert!(g.get(k).is_some(), "faltou `gpu.{k}`");
    }
    // O vendor é slug estável e minúsculo, nunca o `Debug` do enum.
    let Some(Json::Str(vendor)) = g.get("vendor") else { panic!("vendor tem de ser string") };
    assert!(
        ["amd", "intel", "nvidia", "unknown"].contains(&vendor.as_str()),
        "vendor fora do conjunto fechado: {vendor}"
    );

    // **O shape do `vram_cap` NÃO muda de forma conforme o caso.** As cinco chaves estão lá
    // podendo limitar ou não; um objeto que ganha e perde campos obriga a janela a ter dois
    // parsers, e o segundo é o que ninguém testa.
    let c = v.get("vram_cap").unwrap();
    for k in ["limitable", "param", "current_mib", "suggested_mib", "reason"] {
        assert!(c.get(k).is_some(), "faltou `vram_cap.{k}` — o shape não pode variar");
    }
    match c.get("limitable") {
        Some(Json::Bool(true)) => {
            assert_ne!(c.get("param"), Some(&Json::Null), "dá para limitar mas não diz com quê");
            assert_eq!(c.get("reason"), Some(&Json::Null), "não há motivo quando dá");
        }
        Some(Json::Bool(false)) => {
            assert_eq!(c.get("param"), Some(&Json::Null));
            // O motivo é CHAVE de catálogo, não frase — é o que mantém o documento igual em
            // qualquer idioma.
            let Some(Json::Str(r)) = c.get("reason") else { panic!("sem motivo") };
            assert!(r.starts_with("gpu."), "motivo tem de ser chave de catálogo: {r}");
            assert!(!r.contains(' '), "chave de catálogo não tem espaço: {r}");
        }
        outro => panic!("`limitable` é booleano, veio {outro:?}"),
    }
}

/// **O contrato do `limits`**, e os DOIS lados que a tela precisa: o que se sugere e o que já
/// está no disco. Sem os dois não há diff a mostrar antes de aplicar.
#[test]
fn o_shape_do_limits_e_contrato_e_traz_os_dois_lados() {
    let v = parse(&rodar(&["limits"], "en_US.UTF-8"));
    for k in ["optimizer", "cgroup_v2", "boxes", "applied"] {
        assert!(v.get(k).is_some(), "faltou `{k}`");
    }
    let boxes = v.get("boxes").unwrap().arr();
    assert!(!boxes.is_empty(), "sem caixas a tela não tem o que desenhar");
    for b in boxes {
        for k in [
            "name",
            "description",
            "memory_high_mib",
            "memory_max_mib",
            "cpu_quota_pct",
            "io_weight",
        ] {
            assert!(b.get(k).is_some(), "faltou `boxes[].{k}`");
        }
        // A descrição é chave de catálogo (`box.build`), nunca a frase.
        let Some(Json::Str(d)) = b.get("description") else { panic!("description é string") };
        assert!(d.starts_with("box."), "descrição tem de ser chave de catálogo: {d}");
    }
    v.get("applied").unwrap().arr(); // é lista mesmo quando vazia, nunca `null`
}

/// **O contrato do `services`**, e a distinção que ESTE TESTE obrigou o contrato a fazer.
///
/// A primeira versão emitia um campo só, `safe_to_disable`. Este teste reprovou com um caso
/// real desta máquina: `appstream-sync-cache.service` está na allowlist, com os três porquês
/// escritos, mas não apareceu no `blame` — logo não custa nada aqui. Seguro e caro são
/// afirmações diferentes, e a tela precisa das duas:
///
/// - `on_allowlist` — auditado e justificado. É propriedade do serviço.
/// - `recommended` — vale a pena NESTA máquina. É medição, e implica o primeiro.
#[test]
fn o_shape_do_services_e_contrato() {
    let v = parse(&rodar(&["services"], "en_US.UTF-8"));
    for k in ["optimizer", "systemd", "services"] {
        assert!(v.get(k).is_some(), "faltou `{k}`");
    }
    for s in v.get("services").unwrap().arr() {
        for k in
            ["unit", "cost_s", "on_allowlist", "recommended", "does", "why_safe", "what_you_lose"]
        {
            assert!(s.get(k).is_some(), "faltou `services[].{k}`");
        }
        // Quem está na allowlist traz os três porquês; quem não está traz `null` nos três. Um
        // item "seguro" sem justificativa é um item que ninguém auditou.
        let na_lista = s.get("on_allowlist") == Some(&Json::Bool(true));
        for k in ["does", "why_safe", "what_you_lose"] {
            let nulo = s.get(k) == Some(&Json::Null);
            assert_eq!(!nulo, na_lista, "`{k}` e `on_allowlist` têm de andar juntos: {s:?}");
        }
        if na_lista {
            let Some(Json::Str(d)) = s.get("does") else { panic!() };
            assert!(d.starts_with("svc."), "tem de ser chave de catálogo: {d}");
        }
        // Recomendar o que não está na allowlist seria sugerir o que ninguém auditou.
        if s.get("recommended") == Some(&Json::Bool(true)) {
            assert!(na_lista, "recomendado sem estar na allowlist: {s:?}");
        }
    }
}

/// O parser deste arquivo REPROVA o que não é JSON — senão os testes acima passariam sobre
/// qualquer coisa. Guard nunca visto falhando é guard cego.
#[test]
fn o_parser_do_teste_reprova_o_que_e_invalido() {
    for ruim in [
        r#"{"a": "sem aspa}"#,
        r#"{"a" 1}"#,
        r#"{"a": vazio}"#,
        r#"{"a": 1, "a": 2}"#, // chave duplicada: a janela leria a última em silêncio
        "",
    ] {
        let b = ruim.as_bytes();
        let mut i = 0;
        assert!(parse_valor(b, &mut i).is_err(), "devia reprovar: {ruim:?}");
    }
    // E aceita o que é válido — um reprovador que reprova tudo também é cego.
    let mut i = 0;
    assert!(parse_valor(br#"{"a": [1, true, null, "x"]}"#, &mut i).is_ok());
}
