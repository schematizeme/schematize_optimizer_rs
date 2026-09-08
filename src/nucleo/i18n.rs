//! i18n — catálogo de strings da saída do Optimizer.
//!
//! **O quê:** resolve o idioma ativo (env → fallback `en`) e traduz chaves. Os JSONs por
//! idioma vivem em `src/nucleo/i18n/<code>.json` e são embutidos no binário.
//!
//! **Onde:** `t()`/`tf()` na camada `cli`, que é onde este app fala com o usuário.
//!
//! ## Por que este módulo nasceu (ADR-0014, D8)
//!
//! O Optimizer teve os COMANDOS traduzidos para inglês quando virou app próprio, e a SAÍDA
//! ficou em português. Um `--help` em inglês que despeja um relatório em português é a
//! metade estranha do meio caminho: quem não lê português vê a interface prometer algo que
//! o conteúdo não cumpre.
//!
//! ## Por que DOIS idiomas, e não os vinte do market
//!
//! O custo foi medido antes de decidir: são **94 strings distintas** aqui e 53 no Deployer;
//! com a régua de vinte locales seriam ~2.900 traduções. E a saída destes dois apps não é
//! prosa de produto — é `cgroups v2`, `slice` de systemd, `cmdline` de kernel, GTT de GPU.
//! **Uma tradução errada de instrução sobre `cmdline` de kernel custa mais em confiança do
//! que a falta dela**, e ninguém revisaria 2.900 linhas de texto técnico.
//!
//! O catálogo nasce pronto para crescer: acrescentar um idioma é um `.json` e uma linha em
//! [`LANGS`]. E o fallback já cobre o resto — chave ausente no idioma ativo cai em `en`,
//! nunca em texto cru.
//!
//! ## Chave por PARÁGRAFO, nunca por linha
//!
//! A saída deste app é feita de blocos: um título, um bloco explicativo de três linhas, uma
//! instrução. Uma chave por linha impressa pareceria mais simples e seria um anti-padrão de
//! i18n — o tradutor não pode reordenar, e ordem de palavras muda entre idiomas. As chaves
//! aqui carregam o parágrafo inteiro, com `\n` dentro.

use std::collections::HashMap;
use std::sync::RwLock;

/// Idiomas suportados: (código, nome nativo, JSON embutido).
/// Adicionar idioma = um `.json` + uma linha aqui.
pub const LANGS: &[(&str, &str, &str)] = &[
    ("en", "English", include_str!("i18n/en.json")),
    ("pt", "Português", include_str!("i18n/pt.json")),
];

/// Idioma ativo já parseado (mapa do idioma + mapa `en` para fallback).
struct Active {
    code: String,
    map: HashMap<String, String>,
    en: HashMap<String, String>,
}

static ACTIVE: RwLock<Option<Active>> = RwLock::new(None);

/// **O quê:** o código é um idioma que este app tem? **Onde:** [`from_env_value`].
pub fn is_supported(code: &str) -> bool {
    LANGS.iter().any(|(c, _, _)| *c == code)
}

/// **O quê:** JSON embutido de um código, ou o de `en`. **Onde:** [`parse`].
fn json_of(code: &str) -> &'static str {
    LANGS.iter().find(|(c, _, _)| *c == code).map(|(_, _, j)| *j).unwrap_or(LANGS[0].2)
}

/// **O quê:** o mapa de um idioma. **Onde:** [`ensure`].
///
/// JSON inválido devolve mapa VAZIO em vez de panicar: o `t()` então cai no `en`, e o pior
/// caso é a saída em inglês — nunca um app que não abre por causa de um arquivo de tradução.
/// (O teste de paridade é quem reprova JSON quebrado, no lugar certo: o build.)
fn parse(code: &str) -> HashMap<String, String> {
    serde_json::from_str(json_of(code)).unwrap_or_default()
}

/// **O quê:** normaliza um valor de env tipo `pt_BR.UTF-8` → `pt`, se suportado.
/// **Onde:** [`resolve_code`].
fn from_env_value(v: &str) -> Option<String> {
    let two: String = v.chars().take_while(|c| c.is_ascii_alphabetic()).collect();
    let two = two.to_lowercase();
    if two.len() >= 2 {
        let code = &two[..2];
        if is_supported(code) {
            return Some(code.to_string());
        }
    }
    None
}

/// **O quê:** resolve o idioma ativo: `$OPTIMIZER_LANG` → `$SCHEMATIZE_LANG` → `$LC_*`/`$LANG`
/// → `en`.
///
/// **Onde:** [`ensure`], uma vez por execução.
///
/// **Por que `en` no fim, e não `pt`:** este app publica binário para qualquer máquina, e o
/// default de quem não declarou idioma nenhum é a língua franca. Quem quer português já tem
/// `LANG` configurado — foi por isso que a saída em português parecia certa para quem a
/// escreveu e errada para todo o resto.
pub fn resolve_code() -> String {
    for var in ["OPTIMIZER_LANG", "SCHEMATIZE_LANG", "LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(v) = std::env::var(var) {
            if let Some(c) = from_env_value(&v) {
                return c;
            }
        }
    }
    "en".to_string()
}

/// **O quê:** garante o idioma ativo carregado (idempotente). **Onde:** [`t`].
fn ensure() {
    if ACTIVE.read().unwrap().is_some() {
        return;
    }
    let code = resolve_code();
    let a = Active { map: parse(&code), en: parse("en"), code };
    *ACTIVE.write().unwrap() = Some(a);
}

/// **O quê:** código do idioma ativo. **Onde:** diagnóstico.
pub fn current_code() -> String {
    ensure();
    ACTIVE.read().unwrap().as_ref().map(|a| a.code.clone()).unwrap_or_else(|| "en".into())
}

/// **O quê:** traduz uma chave. Fallback: idioma ativo → `en` → a própria chave.
/// **Onde:** toda saída de usuário deste app.
///
/// **Por que a própria chave como último recurso:** uma chave crua na tela (`diag.gpu_header`)
/// é feia e é um bug ÓBVIO, que alguém reporta. Uma string vazia é um bug invisível, e o
/// relatório sai com um buraco que ninguém nota.
pub fn t(key: &str) -> String {
    ensure();
    let g = ACTIVE.read().unwrap();
    let a = g.as_ref().unwrap();
    a.map.get(key).or_else(|| a.en.get(key)).cloned().unwrap_or_else(|| key.to_string())
}

/// **O quê:** traduz e substitui placeholders `{nome}` pelos valores dados.
/// **Onde:** toda linha com dado dentro.
pub fn tf(key: &str, args: &[(&str, &str)]) -> String {
    let mut s = t(key);
    for (k, v) in args {
        s = s.replace(&format!("{{{k}}}"), v);
    }
    s
}

#[cfg(test)]
mod tests_paridade {
    use super::*;
    use std::collections::BTreeSet;

    /// Lê os pares `"chave": "valor"` de um JSON de locale.
    fn pares(bruto: &str) -> Vec<(String, String)> {
        serde_json::from_str::<std::collections::BTreeMap<String, String>>(bruto)
            .expect("locale tem que ser JSON válido")
            .into_iter()
            .collect()
    }

    fn locales() -> Vec<(String, String)> {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/nucleo/i18n");
        let mut v: Vec<(String, String)> = std::fs::read_dir(dir)
            .expect("src/nucleo/i18n existe")
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
            .map(|e| {
                let nome = e.path().file_stem().unwrap().to_string_lossy().to_string();
                (nome, std::fs::read_to_string(e.path()).expect("locale legível"))
            })
            .collect();
        v.sort();
        v
    }

    /// Os placeholders `{assim}` de uma string, ordenados.
    fn placeholders(s: &str) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        let b = s.as_bytes();
        let mut i = 0;
        while let Some(ini) = b[i..].iter().position(|c| *c == b'{').map(|p| p + i) {
            match b[ini..].iter().position(|c| *c == b'}').map(|p| p + ini) {
                Some(fim) => {
                    out.insert(s[ini..=fim].to_string());
                    i = fim + 1;
                }
                None => break,
            }
        }
        out
    }

    /// **O QUÊ:** todo locale tem EXATAMENTE as chaves do `en.json`.
    ///
    /// **Por que `en` é a base, e não `pt`:** `en` é o fallback de toda chave e o default de
    /// quem não declarou idioma. Uma chave que só existe em `pt` é uma chave que a maioria
    /// nunca vê — e o teste passaria se a base fosse a mesma língua da omissão.
    ///
    /// **Por que é teste e não script:** um gate que aponta para um diretório errado é um
    /// gate que ninguém roda. Já aconteceu nesta casa — 18 locales ficaram sem 12 chaves e o
    /// buraco só apareceu por acaso.
    #[test]
    fn todos_os_locales_tem_as_mesmas_chaves() {
        let todos = locales();
        let en = todos.iter().find(|(n, _)| n == "en").expect("en.json existe");
        let base: BTreeSet<String> = pares(&en.1).into_iter().map(|(k, _)| k).collect();
        assert!(!base.is_empty(), "o en.json está vazio");

        for (nome, bruto) in &todos {
            let k: BTreeSet<String> = pares(bruto).into_iter().map(|(k, _)| k).collect();
            let faltam: Vec<_> = base.difference(&k).collect();
            let sobram: Vec<_> = k.difference(&base).collect();
            assert!(
                faltam.is_empty() && sobram.is_empty(),
                "{nome}.json fora de paridade — faltam {faltam:?}, sobram {sobram:?}"
            );
        }
    }

    /// **O QUÊ:** cada tradução carrega os MESMOS placeholders do original.
    ///
    /// **POR QUÊ:** `{parametro}` traduzido ou perdido não quebra a compilação — quebra em
    /// runtime, na cara do usuário, mostrando `{parametro}` cru ou uma frase sem o dado. É o
    /// erro mais fácil de cometer traduzindo e o mais difícil de ver revisando.
    #[test]
    fn traducoes_preservam_os_placeholders() {
        let todos = locales();
        let en: std::collections::BTreeMap<String, String> =
            pares(&todos.iter().find(|(n, _)| n == "en").unwrap().1).into_iter().collect();

        for (nome, bruto) in &todos {
            for (k, v) in pares(bruto) {
                let Some(orig) = en.get(&k) else { continue };
                assert_eq!(
                    placeholders(orig),
                    placeholders(&v),
                    "{nome}.json / {k}: placeholders divergem do en"
                );
            }
        }
    }

    /// **O catálogo nasce pronto para crescer** — a decisão do ADR-0014 (D8) foi dois idiomas
    /// AGORA, não dois idiomas para sempre.
    ///
    /// O teste de paridade acima varre o DIRETÓRIO: ele não sabe quantos locales existem.
    /// Acrescentar um idioma é um `.json` e uma linha em [`LANGS`] — sem tocar em código de
    /// saída. Esta asserção é o que impede alguém de "simplificar" o teste fixando a lista.
    #[test]
    fn acrescentar_idioma_nao_exige_tocar_em_codigo() {
        assert_eq!(LANGS.len(), locales().len(), "todo .json do diretório tem linha em LANGS");
        for (code, nome, _) in LANGS {
            assert!(!code.is_empty() && !nome.is_empty());
            assert!(is_supported(code));
        }
        // E o fallback existe: `en` é o primeiro, e é para ele que `json_of` cai.
        assert_eq!(LANGS[0].0, "en", "o fallback de toda chave é o `en`");
    }

    /// Chave ausente vira a PRÓPRIA CHAVE na tela, nunca string vazia.
    #[test]
    fn chave_desconhecida_aparece_em_vez_de_sumir() {
        assert_eq!(t("nao.existe.esta.chave"), "nao.existe.esta.chave");
    }
}
