//! **jaula** — teto duro de memória POR EXECUÇÃO (descentralizado), fina sobre a lib da casa.
//!
//! **O quê:** traduz o que a pessoa pede (`--classe`, `--mem-mib`, `--tipo`) em um
//! [`Orcamento`] da lib `schematize_plataforma::jaula`, e traduz o [`Veredito`] de volta num
//! [`Desfecho`] (código de saída + status + chave de mensagem). Tudo aqui é PURO, exceto
//! [`coletar`] e [`rodar`], que tocam a máquina.
//!
//! **Onde:** [`crate::cli`] (subcomando `jaula`: `estimar`, `rodar`, `diag`). A lógica mora aqui
//! para ser testável sem subir processo; a camada de CLI só imprime.
//!
//! ## Diferença para as caixas ([`crate::caixas`])
//!
//! As caixas são slices GLOBAIS (`dev-build`, `navegador`, `container`): um teto compartilhado
//! por TUDO o que roda dentro do slice. Se um comando estoura, o OOM escolhe uma vítima entre
//! todos e leva a operação inteira. A jaula é por EXECUÇÃO: cada comando ganha o próprio cgroup
//! com teto de 500 MiB a 2 GiB, e estourar mata SÓ ele, com veredito claro.
//!
//! ## Por que `MemoryHigh` sozinho não basta
//!
//! `MemoryHigh` só faz throttle. Sem swap, a memória anônima não é recuperável: o processo
//! fica engasgado acima do high, sem nunca chegar ao OOM, e trava (a máquina também, se o
//! working set for grande). Por isso a jaula põe `MemoryMax` (corte duro) + `MemorySwapMax=0`
//! e a lib mata a unidade que encalha no high.

use schematize_plataforma::jaula::{
    detectar_backend, estimar, executar, jobs_sugeridos, Backend, Classe, ErroOrcamento, Orcamento,
    TipoExecucao, Veredito,
};
use std::path::{Path, PathBuf};
use std::time::Duration;

#[cfg(test)]
mod testes;

/// Slugs aceitos em `--tipo`, na ordem em que `estimar` os lista.
pub const TIPOS: [&str; 6] = ["agente", "script", "tool", "teste", "build", "navegador"];
/// Slugs aceitos em `--classe`.
pub const CLASSES: [&str; 3] = ["leve", "medio", "pesado"];
/// Código de saída reservado: a execução estourou o teto de memória (convenção de SIGKILL).
pub const EXIT_OOM: i32 = 137;
/// Código de saída: a execução passou do tempo (mesma convenção do `timeout(1)`).
pub const EXIT_TIMEOUT: i32 = 124;
/// Código de saída: não há como impor teto aqui, e nada foi executado.
pub const EXIT_SEM_BACKEND: i32 = 2;

/// **O quê:** slug de `--tipo` → [`TipoExecucao`]. **Onde:** `jaula estimar`.
pub fn tipo_de_slug(s: &str) -> Option<TipoExecucao> {
    Some(match s {
        "agente" => TipoExecucao::AgenteLlm,
        "script" => TipoExecucao::Script,
        "tool" => TipoExecucao::Tool,
        "teste" => TipoExecucao::Teste,
        "build" => TipoExecucao::Build,
        "navegador" => TipoExecucao::Navegador,
        _ => return None,
    })
}

/// **O quê:** slug estável (contrato do `--json`) de um [`TipoExecucao`]. **Onde:** `estimar`.
pub fn slug_tipo(t: TipoExecucao) -> &'static str {
    match t {
        TipoExecucao::AgenteLlm => "agente",
        TipoExecucao::Script => "script",
        TipoExecucao::Tool => "tool",
        TipoExecucao::Teste => "teste",
        TipoExecucao::Build => "build",
        TipoExecucao::Navegador => "navegador",
    }
}

/// **O quê:** slug de `--classe` → [`Classe`]. **Onde:** [`montar_orcamento`].
pub fn classe_de_slug(s: &str) -> Option<Classe> {
    match s {
        "leve" => Some(Classe::Leve),
        "medio" => Some(Classe::Medio),
        "pesado" => Some(Classe::Pesado),
        _ => None,
    }
}

/// **O quê:** slug estável de um [`Backend`]. **Onde:** `jaula diag` e `jaula rodar --json`.
pub fn slug_backend(b: Backend) -> &'static str {
    match b {
        Backend::SystemdScope => "systemd_scope",
        Backend::Prlimit => "prlimit",
        Backend::Nenhum => "none",
    }
}

/// Uma linha de `jaula estimar`: o orçamento de um tipo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinhaEstimativa {
    /// Slug do tipo.
    pub tipo: &'static str,
    /// Slug da classe (pela memória do orçamento).
    pub classe: &'static str,
    /// Teto duro (MiB).
    pub mem_max_mib: u64,
    /// Limiar de pressão (MiB).
    pub mem_high_mib: u64,
    /// Jobs de build sugeridos dentro do teto.
    pub jobs: u64,
}

/// **O quê:** estimativa PURA: um tipo, ou os seis na ordem de [`TIPOS`]. Não executa nada.
/// **Onde:** `jaula estimar`.
pub fn estimativas(tipo: Option<TipoExecucao>) -> Vec<LinhaEstimativa> {
    let todos: Vec<TipoExecucao> = TIPOS.iter().filter_map(|s| tipo_de_slug(s)).collect();
    let alvo = tipo.map_or(todos, |t| vec![t]);
    alvo.into_iter()
        .map(|t| {
            let o = estimar(t);
            let classe = [Classe::Leve, Classe::Medio, Classe::Pesado]
                .into_iter()
                .find(|c| c.mem_mib() == o.mem_max_mib)
                .map_or("medio", slug_da_classe);
            LinhaEstimativa {
                tipo: slug_tipo(t),
                classe,
                mem_max_mib: o.mem_max_mib,
                mem_high_mib: o.mem_high_mib,
                jobs: jobs_sugeridos(&o),
            }
        })
        .collect()
}

/// **O quê:** slug de uma [`Classe`]. **Onde:** [`estimativas`].
fn slug_da_classe(c: Classe) -> &'static str {
    match c {
        Classe::Leve => "leve",
        Classe::Medio => "medio",
        Classe::Pesado => "pesado",
    }
}

/// O que a pessoa pediu em `jaula rodar`, antes de validar.
#[derive(Debug, Clone, Default)]
pub struct Pedido<'a> {
    /// `--classe`.
    pub classe: Option<&'a str>,
    /// `--mem-mib`.
    pub mem_mib: Option<u64>,
    /// `--timeout-s`.
    pub timeout_s: Option<u64>,
    /// `--adr`.
    pub adr: Option<&'a str>,
}

/// Por que um [`Pedido`] foi recusado. Cada variante vira uma mensagem acionável (i18n).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErroPedido {
    /// `--classe` fora de [`CLASSES`].
    ClasseInvalida(String),
    /// `--classe` e `--mem-mib` juntos: dois tetos para a mesma execução.
    ClasseEMem,
    /// `--mem-mib 0`.
    MemZero,
    /// `--timeout-s 0`.
    TimeoutZero,
    /// `--adr` sem `--mem-mib`: não há o que autorizar.
    AdrSemMem,
    /// `--mem-mib` acima de 2048 sem `--adr`.
    AcimaDoTeto(u64),
    /// `--adr` vazio.
    AdrVazio,
}

impl From<ErroOrcamento> for ErroPedido {
    /// **O quê:** converte o erro da lib. **Onde:** [`montar_orcamento`] (via `?`).
    fn from(e: ErroOrcamento) -> Self {
        match e {
            ErroOrcamento::AcimaDoTeto { pedido_mib } => ErroPedido::AcimaDoTeto(pedido_mib),
            ErroOrcamento::AdrVazio => ErroPedido::AdrVazio,
        }
    }
}

impl ErroPedido {
    /// **O quê:** (chave i18n, argumentos) da mensagem. **Onde:** `cli::jaula`.
    pub fn mensagem(&self) -> (&'static str, Vec<(&'static str, String)>) {
        match self {
            ErroPedido::ClasseInvalida(c) => ("jaula.err.classe", vec![("classe", c.clone())]),
            ErroPedido::ClasseEMem => ("jaula.err.classe_e_mem", vec![]),
            ErroPedido::MemZero => ("jaula.err.mem_zero", vec![]),
            ErroPedido::TimeoutZero => ("jaula.err.timeout_zero", vec![]),
            ErroPedido::AdrSemMem => ("jaula.err.adr_sem_mem", vec![]),
            ErroPedido::AcimaDoTeto(m) => ("jaula.err.acima", vec![("mem", m.to_string())]),
            ErroPedido::AdrVazio => ("jaula.err.adr_vazio", vec![]),
        }
    }
}

/// **O quê:** valida um [`Pedido`] e monta o [`Orcamento`]. Sem classe nem memória: `medio`
/// (1024 MiB). `--mem-mib` abaixo de 500 sobe para 500 (piso da lib); acima de 2048 só com ADR.
/// **Onde:** `jaula rodar`, antes de qualquer execução.
pub fn montar_orcamento(p: &Pedido) -> Result<Orcamento, ErroPedido> {
    if p.adr.is_some() && p.mem_mib.is_none() {
        return Err(ErroPedido::AdrSemMem);
    }
    let mut o = match (p.classe, p.mem_mib) {
        (Some(_), Some(_)) => return Err(ErroPedido::ClasseEMem),
        (Some(c), None) => Orcamento::da_classe(
            classe_de_slug(c).ok_or_else(|| ErroPedido::ClasseInvalida(c.to_string()))?,
        ),
        (None, Some(0)) => return Err(ErroPedido::MemZero),
        (None, Some(m)) => Orcamento::novo(m, p.adr.map(str::to_string))?,
        (None, None) => Orcamento::da_classe(Classe::Medio),
    };
    if let Some(s) = p.timeout_s {
        if s == 0 {
            return Err(ErroPedido::TimeoutZero);
        }
        o.timeout = Some(Duration::from_secs(s));
    }
    Ok(o)
}

/// O fim de uma execução jaulada, já traduzido para o que o optimizer devolve.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Desfecho {
    /// Código de saída do optimizer.
    pub codigo: i32,
    /// Slug estável do veredito (contrato do `--json`).
    pub status: &'static str,
    /// Teto que valia (MiB).
    pub teto_mib: u64,
    /// Pico de memória observado (MiB), se medido.
    pub pico_mib: Option<u64>,
    /// Chave i18n da mensagem humana (`None` quando concluiu).
    pub chave: Option<&'static str>,
    /// Argumentos da mensagem.
    pub args: Vec<(&'static str, String)>,
}

/// **O quê:** mapeamento PURO Veredito → [`Desfecho`]: Concluiu→0; Falhou(c)→c (0 vira 1, nunca
/// mascara falha); EstourouMemoria→137; Timeout→124; SemBackend→2.
/// **Onde:** [`rodar`] e os testes.
pub fn desfecho(v: &Veredito, teto_mib: u64, timeout_s: Option<u64>) -> Desfecho {
    let base = |codigo, status| Desfecho {
        codigo,
        status,
        teto_mib,
        pico_mib: None,
        chave: None,
        args: vec![],
    };
    match v {
        Veredito::Concluiu(_) => base(0, "completed"),
        Veredito::Falhou(c) => {
            let codigo = if *c == 0 { 1 } else { *c };
            let args = vec![("code", codigo.to_string())];
            Desfecho { chave: Some("jaula.rodar.falhou"), args, ..base(codigo, "failed") }
        }
        Veredito::EstourouMemoria { pico_mib, teto_mib: t } => Desfecho {
            teto_mib: *t,
            pico_mib: *pico_mib,
            chave: Some("jaula.rodar.oom"),
            args: vec![("teto", t.to_string())],
            ..base(EXIT_OOM, "oom_killed")
        },
        Veredito::Timeout => Desfecho {
            chave: Some("jaula.rodar.timeout"),
            args: vec![("s", timeout_s.unwrap_or(0).to_string())],
            ..base(EXIT_TIMEOUT, "timeout")
        },
        Veredito::SemBackend(_) => Desfecho {
            chave: Some("jaula.rodar.sem_backend"),
            ..base(EXIT_SEM_BACKEND, "no_backend")
        },
    }
}

/// **O quê:** no modo `--json` o stdout é do documento; faz o filho escrever o stdout dele no
/// stderr (`exec "$@" 1>&2`), com argv puro (nada é interpolado no script).
/// **Onde:** [`rodar`].
pub fn argv_para_json(argv: &[String]) -> Vec<String> {
    let mut v: Vec<String> =
        ["sh", "-c", r#"exec "$@" 1>&2"#, "jaula"].iter().map(|s| s.to_string()).collect();
    v.extend(argv.iter().cloned());
    v
}

/// **O quê:** executa `argv` na jaula e devolve o [`Desfecho`] (mais o backend usado).
/// **Onde:** `cli::jaula::rodar_cmd`. Sem backend a lib NÃO executa (nunca roda sem teto).
pub fn rodar(o: &Orcamento, argv: &[String], nome: &str, json: bool) -> (Desfecho, Backend) {
    let backend = detectar_backend();
    let efetivo = if json { argv_para_json(argv) } else { argv.to_vec() };
    let v = executar(o, &efetivo, nome);
    (desfecho(&v, o.mem_max_mib, o.timeout.map(|t| t.as_secs())), backend)
}

/// O que a máquina tem para sustentar a jaula (fatos crus, sem decisão).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fatos {
    /// cgroup v2 montado.
    pub cgroup_v2: bool,
    /// controller `memory` delegado ao manager do usuário.
    pub controller_memory: bool,
    /// `systemd-run` no PATH.
    pub systemd_run: bool,
    /// Sessão de usuário (`XDG_RUNTIME_DIR`).
    pub bus_usuario: bool,
    /// `prlimit` no PATH.
    pub prlimit: bool,
}

/// **O quê:** o que FALTA para ativar o backend forte (systemd scope), como slugs fechados.
/// **Onde:** `jaula diag` (humano e `--json`).
pub fn faltas(f: &Fatos) -> Vec<&'static str> {
    let mut v = Vec::new();
    if !f.cgroup_v2 {
        v.push("cgroup_v2");
    }
    if !f.controller_memory {
        v.push("memory_controller");
    }
    if !f.systemd_run {
        v.push("systemd_run");
    }
    if !f.bus_usuario {
        v.push("user_session");
    }
    v
}

/// **O quê:** `true` se `bin` é arquivo em algum diretório do PATH. **Onde:** [`coletar`].
fn no_path(bin: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|p| std::env::split_paths(&p).any(|d| d.join(bin).is_file()))
}

/// **O quê:** uid real lido de `/proc/self/status`. **Onde:** [`coletar`].
fn uid_atual() -> Option<u32> {
    let s = std::fs::read_to_string("/proc/self/status").ok()?;
    s.lines().find(|l| l.starts_with("Uid:"))?.split_whitespace().nth(1)?.parse().ok()
}

/// **O quê:** `cgroup.controllers` do manager do usuário. **Onde:** [`coletar`].
fn controllers_do_usuario(uid: u32) -> PathBuf {
    Path::new("/sys/fs/cgroup/user.slice")
        .join(format!("user-{uid}.slice/user@{uid}.service/cgroup.controllers"))
}

/// **O quê:** sonda a máquina (só leitura). **Onde:** `jaula diag`.
pub fn coletar() -> Fatos {
    let controllers =
        uid_atual().and_then(|u| std::fs::read_to_string(controllers_do_usuario(u)).ok());
    Fatos {
        cgroup_v2: crate::diag::maquina::cgroup_v2(Path::new("/sys/fs/cgroup")),
        controller_memory: controllers.is_some_and(|c| c.split_whitespace().any(|x| x == "memory")),
        systemd_run: no_path("systemd-run"),
        bus_usuario: std::env::var_os("XDG_RUNTIME_DIR").is_some(),
        prlimit: no_path("prlimit"),
    }
}
