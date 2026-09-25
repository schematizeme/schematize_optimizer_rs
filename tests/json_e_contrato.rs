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
    /// **O quê:** o texto, se isto for string. **Onde:** as asserções sobre SLUGS.
    ///
    /// Devolve `None` para qualquer outro tipo, em vez de converter: um número que virasse
    /// `"8"` faria uma asserção de slug passar sobre um campo que não é slug nenhum.
    fn texto(&self) -> Option<String> {
        match self {
            Json::Str(s) => Some(s.clone()),
            _ => None,
        }
    }
    /// **O quê:** o número, se isto for número. **Onde:** a asserção da invariante dos tetos.
    fn numero(&self) -> Option<f64> {
        match self {
            Json::Num(n) => Some(*n),
            _ => None,
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

/// **Todo subcomando com `--json`, e a lista é UMA só.**
///
/// Ela existe aqui, e não repetida em cada teste, porque a fase E3 do ADR-0018 acrescentou dois
/// subcomandos — e, com a lista repetida, acrescentar um significaria lembrar de três lugares.
/// Esquecer um deixaria o contrato novo sem o teste de idioma, que é justamente o que este
/// arquivo existe para travar.
const SUBCOMANDOS: &[&[&str]] = &[&["diag"], &["limits"], &["services"], &["disco"], &["agentes"]];

/// Os subcomandos cujo documento é ESTÁVEL entre duas invocações seguidas.
///
/// **O `agentes` fica de fora, e não é omissão:** ele carrega `load1` e `running_claudes`, que
/// mudam ENTRE UMA CHAMADA E OUTRA por definição — o load average é uma média móvel. Compará-lo
/// byte a byte é um teste que passa sozinho e falha na suíte cheia, que é a definição de flaky.
/// Descobri isso do jeito certo: ele passou isolado e reprovou no `--all-targets`.
///
/// **A regra que o `agentes` tem de cumprir está em `o_agentes_nao_tem_prosa_para_traduzir`**, e
/// é mais forte que igualdade de bytes: o documento não tem NENHUMA string traduzível, então não
/// há o que mudar com o idioma.
const SUBCOMANDOS_ESTAVEIS: &[&[&str]] = &[&["diag"], &["limits"], &["services"], &["disco"]];

/// **O BUG QUE ESTE ARQUIVO TRAVA.** O documento é o mesmo em qualquer idioma — byte a byte.
///
/// Se um dia alguém puser uma frase traduzida num campo (`"reason": "no Intel a memória…"`),
/// este teste falha na hora, e não seis meses depois na máquina de um usuário japonês.
#[test]
fn os_json_sao_byte_a_byte_iguais_em_qualquer_idioma() {
    for sub in SUBCOMANDOS_ESTAVEIS {
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
fn os_json_sao_validos_e_nao_tem_sobra() {
    for sub in SUBCOMANDOS {
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

// ---------------------------------------------------------------------------
// DISCO e AGENTES — chegaram na fase E3 do ADR-0018, vindos do app principal.
// ---------------------------------------------------------------------------

/// **O contrato do `disco`.** Estas chaves são lidas pela aba Disco da janela.
///
/// O documento traz os TRÊS cortes da mesma varredura (por disco, por tipo, e os achados)
/// porque fazer a janela recalcular os agregados seria duas somas do mesmo número — e a que
/// divergisse mostraria um total que não bate com as linhas.
#[test]
fn o_shape_do_disco_e_contrato() {
    let v = parse(&rodar(&["disco"], "en_US.UTF-8"));
    for k in
        ["optimizer", "total_bytes", "por_montagem", "por_tipo", "achados", "docker_disponivel"]
    {
        assert!(v.get(k).is_some(), "falta a chave `{k}` no contrato do disco");
    }
    // `docker_disponivel` é SEPARADO da lista vazia: máquina sem docker e máquina com docker e
    // nada a recuperar são estados diferentes, e a janela desenha os dois de formas diferentes.
    assert!(v.get("docker").is_some(), "a lista do docker existe mesmo quando vazia");

    // Os achados, quando há: o slug do tipo é conjunto FECHADO, e `bytes` é NÚMERO.
    if let Some(a) = v.get("achados").map(|x| x.arr()).and_then(|a| a.first()) {
        for k in ["caminho", "tipo", "bytes", "dias_parado", "montagem", "refaz_text"] {
            assert!(a.get(k).is_some(), "falta `{k}` num achado");
        }
    }
}

/// **O slug do tipo NÃO é o rótulo humano.**
///
/// O `rotulo()` é prosa (`"target (Rust)"`, com espaço e parêntese) e muda de redação; o slug é
/// o que a janela casa para escolher ícone e filtro. Confundir os dois é o defeito que este
/// projeto já pagou duas vezes — e o teste o impede por CONSTRUÇÃO: um slug com espaço ou
/// parêntese reprova.
#[test]
fn o_slug_do_tipo_de_disco_e_estavel_e_nao_e_prosa() {
    let v = parse(&rodar(&["disco"], "en_US.UTF-8"));
    let conhecidos = [
        "rust-target",
        "node-modules",
        "node-build",
        "npm-cache",
        "go-cache",
        "python-cache",
        "python-venv",
        "cargo-cache",
    ];
    for t in v.get("por_tipo").map(|x| x.arr()).unwrap_or(&[]) {
        let slug = t.get("tipo").and_then(|x| x.texto()).unwrap_or_default();
        assert!(
            conhecidos.contains(&slug.as_str()),
            "`{slug}` não é um slug conhecido — o conjunto é FECHADO e a janela ramifica por ele"
        );
        assert!(
            !slug.contains(' ') && !slug.contains('('),
            "`{slug}` parece PROSA, não slug — o rótulo humano vazou para o contrato"
        );
    }
}

/// **O contrato do `agentes`, e por que os TRÊS tetos entram.**
///
/// O `total_cap` é o menor deles. Um documento só com o resultado obrigaria a janela a
/// adivinhar qual recurso está apertando — ou a recalcular, que é duas leis para o mesmo número.
#[test]
fn o_shape_do_agentes_e_contrato_e_traz_os_tres_tetos() {
    let v = parse(&rodar(&["agentes"], "en_US.UTF-8"));
    for k in [
        "optimizer",
        "threads",
        "mem_available_mb",
        "load1",
        "running_claudes",
        "cpu_cap",
        "ram_cap",
        "load_cap",
        "total_cap",
        "available",
        "ram_tight",
    ] {
        assert!(v.get(k).is_some(), "falta a chave `{k}` no contrato dos agentes");
    }
    // A invariante do domínio, afirmada no CONTRATO: o teto final é o menor dos três.
    let n = |k: &str| v.get(k).and_then(|x| x.numero()).unwrap_or(-1.0);
    let menor = n("cpu_cap").min(n("ram_cap")).min(n("load_cap"));
    assert_eq!(n("total_cap"), menor, "o teto total tem de ser o MENOR dos três");
    assert!(n("available") <= n("total_cap"), "disponível nunca passa do teto");
    assert!(n("available") >= 0.0, "disponível nunca é negativo");
}

/// **O `agentes` não tem prosa para traduzir — e isso é mais forte que igualdade de bytes.**
///
/// Ele não entra no teste byte a byte porque `load1` e `running_claudes` mudam entre duas
/// chamadas (o load é média móvel). A garantia equivalente é estrutural: se o documento não tem
/// NENHUMA string além da versão, não há o que um catálogo de tradução possa alcançar.
#[test]
fn o_agentes_nao_tem_prosa_para_traduzir() {
    let v = parse(&rodar(&["agentes"], "en_US.UTF-8"));
    let Json::Obj(m) = &v else { panic!("o documento é um objeto") };
    for (k, valor) in m {
        if k == "optimizer" {
            continue; // a versão é string por natureza, e não é prosa
        }
        assert!(
            valor.texto().is_none(),
            "`{k}` é uma STRING no contrato do agentes — prosa aqui é o que um dia vira \
             tradução, e aí o documento muda com o idioma"
        );
    }
}

/// **`refaz_text` é o único campo de prosa do `disco`, e o `_text` no nome é a declaração.**
///
/// Precedente do `status_text` do market. O teste existe para impedir que um SEGUNDO campo de
/// prosa entre sem o sufixo — porque o sufixo é o que avisa quem lê que ali não se ramifica.
#[test]
fn a_unica_prosa_do_disco_se_declara_no_nome() {
    let v = parse(&rodar(&["disco"], "en_US.UTF-8"));
    for a in v.get("achados").map(|x| x.arr()).unwrap_or(&[]) {
        let Json::Obj(m) = a else { panic!("cada achado é um objeto") };
        for (k, valor) in m {
            if valor.texto().is_none() || k.ends_with("_text") {
                continue;
            }
            // Caminho e slug são strings, mas NÃO são prosa: um é identificador do sistema de
            // arquivos, o outro é conjunto fechado. Os dois são estáveis entre idiomas.
            assert!(
                ["caminho", "montagem", "tipo"].contains(&k.as_str()),
                "`{k}` é uma string nova no contrato do disco. Se for prosa, o nome tem de \
                 terminar em `_text`; se for decisão, tem de ser slug de conjunto fechado"
            );
        }
    }
}
