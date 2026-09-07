//! DIAGNÓSTICO — RAM, núcleos e o que sustenta as sugestões.
//!
//! **O quê:** lê os números que as regras de [`super::gpu`] e `caixas` consomem.
//!
//! **Onde:** `optimizer diag`. Cada leitura recebe a raiz como parâmetro para o teste poder
//! montar um `/proc` de mentira — do contrário o teste seria "funciona no meu computador".

use std::path::Path;

/// O retrato da máquina.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Maquina {
    pub ram_bytes: u64,
    pub nucleos: u32,
}

/// **O quê:** extrai o `MemTotal` (em kB) de um `/proc/meminfo`. Função PURA sobre o texto.
/// **Onde:** [`detectar_em`] e o teste.
pub fn ram_de_meminfo(txt: &str) -> Option<u64> {
    for l in txt.lines() {
        if let Some(r) = l.strip_prefix("MemTotal:") {
            let kb: u64 = r.split_whitespace().next()?.parse().ok()?;
            return Some(kb * 1024);
        }
    }
    None
}

/// **O quê:** conta os núcleos num `/proc/cpuinfo`. Função PURA sobre o texto.
pub fn nucleos_de_cpuinfo(txt: &str) -> u32 {
    txt.lines().filter(|l| l.starts_with("processor")).count() as u32
}

/// **O quê:** o retrato, a partir de uma raiz de `/proc`.
pub fn detectar_em(proc_raiz: &Path) -> Maquina {
    let ram = std::fs::read_to_string(proc_raiz.join("meminfo"))
        .ok()
        .and_then(|s| ram_de_meminfo(&s))
        .unwrap_or(0);
    let nuc = std::fs::read_to_string(proc_raiz.join("cpuinfo"))
        .map(|s| nucleos_de_cpuinfo(&s))
        .unwrap_or(1)
        .max(1);
    Maquina { ram_bytes: ram, nucleos: nuc }
}

/// **O quê:** o retrato desta máquina.
pub fn detectar() -> Maquina {
    detectar_em(Path::new("/proc"))
}

/// **O quê:** cgroups v2 está montado? Sem ele, as caixas não funcionam — e dizer isso ANTES
/// é melhor que gravar slices que o systemd vai ignorar em silêncio.
pub fn cgroup_v2(raiz: &Path) -> bool {
    raiz.join("cgroup.controllers").is_file()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// O parser lê o formato real do `/proc/meminfo`, e devolve BYTES (o arquivo dá kB).
    #[test]
    fn le_o_memtotal_e_converte_para_bytes() {
        let txt = "MemTotal:       16283492 kB\nMemFree:         2465132 kB\n";
        assert_eq!(ram_de_meminfo(txt), Some(16283492 * 1024));
    }

    /// Arquivo sem `MemTotal` devolve `None` — não zero, que pareceria "máquina sem RAM" e
    /// faria as sugestões saírem absurdas.
    #[test]
    fn meminfo_sem_memtotal_e_none() {
        assert_eq!(ram_de_meminfo("MemFree: 100 kB\n"), None);
        assert_eq!(ram_de_meminfo(""), None);
    }

    /// Conta uma linha `processor` por núcleo lógico.
    #[test]
    fn conta_os_nucleos() {
        let txt = "processor\t: 0\nmodel name\t: x\n\nprocessor\t: 1\nmodel name\t: x\n";
        assert_eq!(nucleos_de_cpuinfo(txt), 2);
        assert_eq!(nucleos_de_cpuinfo(""), 0);
    }

    /// `/proc` ausente não faz o app explodir: núcleos cai em 1, que é seguro para as regras.
    #[test]
    fn proc_ausente_degrada_em_vez_de_quebrar() {
        let m = detectar_em(Path::new("/caminho/que/nao/existe"));
        assert_eq!(m.ram_bytes, 0);
        assert_eq!(m.nucleos, 1, "nunca zero: uma divisão por núcleos exploderia");
    }
}
