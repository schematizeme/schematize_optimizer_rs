//! Testes de ponta a ponta de `optimizer jaula`: o binário REAL, com cgroup REAL quando há.
//!
//! **O quê:** prova o contrato do subcomando: códigos de saída (0, filho, 137, 124, 2), recusas
//! de pedido hostil, o shape do `--json`, e — o ponto da feature — que estourar o teto mata SÓ
//! a execução jaulada (um processo vizinho segue vivo).
//!
//! **Sem backend** (sem systemd scope com `memory` delegado) os testes que dependem dele NÃO são
//! ignorados em silêncio: avisam no stderr e, onde dá, provam o caminho `SemBackend` (exit 2).

use serde_json::Value;
use std::process::{Child, Command, Output, Stdio};

/// Variável que transforma o teste-filho [`alocador_filho`] num alocador de memória.
const ENV_ALOCA: &str = "JAULA_TESTE_ALOCA_MIB";

/// **O quê:** caminho do binário do optimizer. **Onde:** [`rodar`].
fn bin() -> std::path::PathBuf {
    let mut p = std::env::current_exe().expect("o teste tem caminho");
    p.pop();
    if p.ends_with("deps") {
        p.pop();
    }
    p.join("schematize-optimizer")
}

/// **O quê:** roda o optimizer com idioma controlado (`LANG` herdado nunca vence o do teste).
/// **Onde:** todos os testes daqui.
fn rodar(args: &[&str], lang: &str, envs: &[(&str, &str)]) -> Output {
    let mut c = Command::new(bin());
    for v in ["OPTIMIZER_LANG", "SCHEMATIZE_LANG", "LC_ALL", "LC_MESSAGES", "LANG", "LANGUAGE"] {
        c.env_remove(v);
    }
    c.args(args)
        .envs(envs.iter().copied())
        .env("LANG", lang)
        .stdin(Stdio::null())
        .output()
        .unwrap_or_else(|e| panic!("não consegui executar {}: {e}", bin().display()))
}

/// **O quê:** texto UTF-8 de um fluxo. **Onde:** asserções.
fn txt(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

/// **O quê:** o backend detectado, pelo próprio `jaula diag --json`. **Onde:** [`com_backend`].
fn backend() -> String {
    let o = rodar(&["jaula", "diag", "--json"], "en_US.UTF-8", &[]);
    let v: Value = serde_json::from_slice(&o.stdout).expect("diag --json é JSON");
    v["backend"].as_str().expect("backend é string").to_string()
}

/// **O quê:** `true` se há backend de cgroup; senão avisa (nunca pula mudo). **Onde:** testes
/// que dependem de OOM real.
fn com_cgroup(teste: &str) -> bool {
    let b = backend();
    if b != "systemd_scope" {
        eprintln!("PULEI {teste}: backend `{b}` não prova OOM (precisa de systemd_scope)");
    }
    b == "systemd_scope"
}

/// **O quê:** o "alocador": quando `JAULA_TESTE_ALOCA_MIB` está setada, aloca e TOCA essa
/// memória e dorme; sem a variável, passa sem fazer nada. **Onde:** é o comando que o teste
/// de OOM manda a jaula executar (o próprio executável de teste, `--exact alocador_filho`).
#[test]
fn alocador_filho() {
    let Some(mib) = std::env::var(ENV_ALOCA).ok().and_then(|v| v.parse::<usize>().ok()) else {
        return;
    };
    let mut v = vec![0u8; mib * 1024 * 1024];
    for i in (0..v.len()).step_by(4096) {
        v[i] = 1;
    }
    std::hint::black_box(&v);
    std::thread::sleep(std::time::Duration::from_secs(30));
}

/// **O quê:** sobe um processo VIZINHO (fora da jaula). **Onde:** prova de confinamento.
fn vizinho() -> Child {
    Command::new("sleep").arg("60").stdin(Stdio::null()).spawn().expect("sleep existe")
}

/// **O quê:** o executável de teste como argv do alocador. **Onde:** testes de OOM.
fn argv_alocador() -> Vec<String> {
    let exe = std::env::current_exe().expect("exe de teste").to_string_lossy().into_owned();
    vec![exe, "--exact".into(), "alocador_filho".into(), "--nocapture".into()]
}

/// **O quê:** monta `jaula rodar <flags> -- <cmd>`. **Onde:** todos os testes de `rodar`.
fn linha<'a>(flags: &[&'a str], cmd: &'a [String]) -> Vec<&'a str> {
    let mut v = vec!["jaula", "rodar"];
    v.extend_from_slice(flags);
    v.push("--");
    v.extend(cmd.iter().map(String::as_str));
    v
}

/// O ponto da feature: 800 MiB com teto de 500 → exit 137, mensagem clara, e o vizinho vive.
#[test]
fn oom_real_sai_137_e_so_a_jaula_morre() {
    if !com_cgroup("oom_real_sai_137_e_so_a_jaula_morre") {
        return;
    }
    let mut viz = vizinho();
    let cmd = argv_alocador();
    let args = linha(&["--mem-mib", "500", "--timeout-s", "120", "--nome", "t-oom"], &cmd);
    let o = rodar(&args, "en_US.UTF-8", &[(ENV_ALOCA, "800")]);
    let vivo = viz.try_wait().expect("try_wait").is_none();
    viz.kill().ok();
    viz.wait().ok();
    assert_eq!(o.status.code(), Some(137), "stderr: {}", txt(&o.stderr));
    let e = txt(&o.stderr);
    assert!(e.contains("went over 500 MiB") && e.contains("rest of the machine"), "{e}");
    assert!(vivo, "o processo vizinho morreu junto: a jaula não confinou");
    let o = rodar(&args, "pt_BR.UTF-8", &[(ENV_ALOCA, "800")]);
    assert_eq!(o.status.code(), Some(137));
    assert!(txt(&o.stderr).contains("passou de 500 MiB e foi encerrada sozinha"));
}

/// O `--json` do OOM: shape fechado, status em slug, sem prosa, stdout só o documento.
#[test]
fn json_do_oom_tem_shape_e_nao_tem_prosa() {
    if !com_cgroup("json_do_oom_tem_shape_e_nao_tem_prosa") {
        return;
    }
    let cmd = argv_alocador();
    let args =
        linha(&["--mem-mib", "500", "--timeout-s", "120", "--nome", "t-json", "--json"], &cmd);
    let o = rodar(&args, "en_US.UTF-8", &[(ENV_ALOCA, "800")]);
    assert_eq!(o.status.code(), Some(137));
    let v: Value = serde_json::from_slice(&o.stdout).expect("stdout é UM documento JSON");
    assert_eq!(v["status"], "oom_killed");
    assert_eq!(v["exit_code"], 137);
    assert_eq!(v["ceiling_mib"], 500);
    assert_eq!(v["backend"], "systemd_scope");
    assert_eq!(v["name"], "t-json");
    assert!(v["peak_mib"].is_null() || v["peak_mib"].is_u64());
    assert_eq!(v["optimizer"], env!("CARGO_PKG_VERSION"));
    let pt = rodar(&args, "pt_BR.UTF-8", &[(ENV_ALOCA, "800")]);
    // `peak_mib` é medição e varia entre duas execuções; o resto tem de ser idêntico.
    let mut b: Value = serde_json::from_slice(&pt.stdout).expect("JSON válido");
    let mut a = v.clone();
    a["peak_mib"] = Value::Null;
    b["peak_mib"] = Value::Null;
    assert_eq!(a, b, "o JSON mudou com o idioma");
}

/// Concluir é 0; o código do filho passa; o stdout do filho vai ao stderr no modo `--json`.
#[test]
fn sucesso_codigo_do_filho_e_stdout_do_json() {
    if !com_cgroup("sucesso_codigo_do_filho_e_stdout_do_json") {
        return;
    }
    let ok = vec!["true".to_string()];
    assert_eq!(rodar(&linha(&["--classe", "leve"], &ok), "en", &[]).status.code(), Some(0));
    let sai7 = vec!["sh".to_string(), "-c".into(), "exit 7".into()];
    assert_eq!(rodar(&linha(&[], &sai7), "en", &[]).status.code(), Some(7));
    let eco = vec!["echo".to_string(), "ruido-do-filho".into()];
    let o = rodar(&linha(&["--json"], &eco), "en", &[]);
    let v: Value = serde_json::from_slice(&o.stdout).expect("stdout é só o documento");
    assert_eq!((v["status"].as_str(), v["exit_code"].as_i64()), (Some("completed"), Some(0)));
    assert!(txt(&o.stderr).contains("ruido-do-filho"), "o stdout do filho devia ir ao stderr");
}

/// Timeout é 124, e mata só a unidade.
#[test]
fn timeout_sai_124() {
    if !com_cgroup("timeout_sai_124") {
        return;
    }
    let cmd = vec!["sleep".to_string(), "30".into()];
    let o = rodar(&linha(&["--timeout-s", "1"], &cmd), "en_US.UTF-8", &[]);
    assert_eq!(o.status.code(), Some(124), "stderr: {}", txt(&o.stderr));
    assert!(txt(&o.stderr).contains("went over 1 s"));
}

/// Sem backend o optimizer NÃO roda e sai 2 com mensagem acionável; com backend, este teste
/// registra que o caminho não se aplica (nunca silêncio).
#[test]
fn sem_backend_nao_roda_e_sai_2() {
    if backend() != "none" {
        eprintln!("PULEI sem_backend_nao_roda_e_sai_2: esta máquina TEM backend");
        return;
    }
    let cmd = vec!["true".to_string()];
    let o = rodar(&linha(&[], &cmd), "en_US.UTF-8", &[]);
    assert_eq!(o.status.code(), Some(2));
    assert!(txt(&o.stderr).contains("jaula diag"));
}

/// Pedidos hostis: recusados antes de executar, com exit 1 e a explicação acionável.
#[test]
fn pedidos_hostis_sao_recusados_com_explicacao() {
    let cmd = vec!["true".to_string()];
    let casos: [(&[&str], &str); 4] = [
        (&["--mem-mib", "4096"], "--adr <id>"),
        (&["--mem-mib", "0"], "minimum is 500"),
        (&["--timeout-s", "0"], "give 1 or more"),
        (&["--mem-mib", "4096", "--adr", ""], "`--adr` is empty"),
    ];
    for (flags, trecho) in casos {
        let o = rodar(&linha(flags, &cmd), "en_US.UTF-8", &[]);
        assert_eq!(o.status.code(), Some(1), "{flags:?}: {}", txt(&o.stderr));
        assert!(txt(&o.stderr).contains(trecho), "{flags:?}: {}", txt(&o.stderr));
        assert!(txt(&o.stdout).is_empty(), "recusa não escreve no stdout");
    }
}

/// Argumento malformado nunca vira teto: o clap recusa (exit 2) e nada executa.
#[test]
fn args_malformados_sao_recusados_pelo_clap() {
    let cmd = vec!["true".to_string()];
    let casos: [&[&str]; 6] = [
        &["--classe", "gigante"],
        &["--classe", "leve", "--mem-mib", "600"],
        &["--adr", "ADR-1"],
        &["--mem-mib", "-5"],
        &["--mem-mib", "99999999999999999999999999"],
        &["--timeout-s", "abc"],
    ];
    for flags in casos {
        let o = rodar(&linha(flags, &cmd), "en_US.UTF-8", &[]);
        assert_eq!(o.status.code(), Some(2), "{flags:?} devia ser recusado");
    }
    let sem_cmd = rodar(&["jaula", "rodar", "--classe", "leve", "--"], "en", &[]);
    assert_eq!(sem_cmd.status.code(), Some(2), "sem comando não há o que jaular");
    let sem_traco = rodar(&["jaula", "rodar", "true"], "en", &[]);
    assert_eq!(sem_traco.status.code(), Some(2), "o comando exige `--`");
}

/// `estimar --json`: shape fechado, igual em qualquer idioma, e não executa nada.
#[test]
fn estimar_json_tem_shape_e_e_igual_em_qualquer_idioma() {
    let en = rodar(&["jaula", "estimar", "--json"], "en_US.UTF-8", &[]);
    let pt = rodar(&["jaula", "estimar", "--json"], "pt_BR.UTF-8", &[]);
    assert!(en.status.success());
    assert_eq!(en.stdout, pt.stdout, "o JSON mudou com o idioma");
    let v: Value = serde_json::from_slice(&en.stdout).expect("JSON válido");
    let tipos = v["types"].as_array().expect("types é lista");
    assert_eq!(tipos.len(), 6);
    for t in tipos {
        for k in ["type", "class", "mem_max_mib", "mem_high_mib", "jobs"] {
            assert!(t.get(k).is_some(), "falta `{k}`");
        }
    }
    let um = rodar(&["jaula", "estimar", "--tipo", "build", "--json"], "en", &[]);
    let v: Value = serde_json::from_slice(&um.stdout).expect("JSON válido");
    let b = &v["types"][0];
    assert_eq!(
        (b["class"].as_str(), b["mem_max_mib"].as_u64(), b["jobs"].as_u64()),
        (Some("pesado"), Some(2048), Some(2))
    );
    assert_eq!(v["types"].as_array().map(Vec::len), Some(1));
    let ruim = rodar(&["jaula", "estimar", "--tipo", "foguete"], "en", &[]);
    assert_eq!(ruim.status.code(), Some(2));
}

/// `estimar` humano: sem chave crua na tela e dizendo que nada foi executado.
#[test]
fn estimar_humano_traduz_e_nao_vaza_chave() {
    for (lang, trecho) in
        [("en_US.UTF-8", "nothing was executed"), ("pt_BR.UTF-8", "nada foi executado")]
    {
        let o = rodar(&["jaula", "estimar"], lang, &[]);
        let s = txt(&o.stdout);
        assert!(o.status.success() && s.contains(trecho), "{s}");
        assert!(!s.contains("jaula."), "chave crua na tela: {s}");
    }
}

/// `diag --json`: shape fechado e coerente (`missing` bate com os fatos); humano sem chave crua.
#[test]
fn diag_json_tem_shape_e_o_humano_nao_vaza_chave() {
    let o = rodar(&["jaula", "diag", "--json"], "en", &[]);
    let v: Value = serde_json::from_slice(&o.stdout).expect("JSON válido");
    for k in ["backend", "cgroup_v2", "memory_controller", "systemd_run", "user_session", "prlimit"]
    {
        assert!(v.get(k).is_some(), "falta `{k}`");
    }
    assert!(["systemd_scope", "prlimit", "none"].contains(&v["backend"].as_str().unwrap_or("")));
    let faltam = v["missing"].as_array().expect("missing é lista");
    let esperado = faltam.iter().any(|m| m == "memory_controller");
    assert_eq!(esperado, v["memory_controller"] == false);
    let h = txt(&rodar(&["jaula", "diag"], "pt_BR.UTF-8", &[]).stdout);
    assert!(h.contains("backend:") && !h.contains("jaula."), "{h}");
}
