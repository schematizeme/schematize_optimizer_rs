//! DIAGNÓSTICO — a memória que a GPU pode tomar do sistema.
//!
//! **O quê:** descobre o vendor da GPU, quanta RAM ela pode mapear (GTT) e se **esta máquina**
//! aceita limitar isso.
//!
//! **Onde:** `optimizer diag` e, adiante, a sugestão de `amdgpu.gttsize`.
//!
//! ## O que é GTT, e por que ele é o número que importa
//!
//! GTT (*Graphics Translation Table*) é a **RAM do sistema** que a GPU pode mapear para si. Não
//! é memória "roubada" no sentido de reservada para sempre — é um **teto**. Mas o `amdgpu`
//! define esse teto como **metade da RAM** por padrão, e é isso que tira a previsibilidade:
//! num pico, metade da máquina pode ir para a GPU enquanto um build precisa dela.
//!
//! Limitar o teto é o que o pedido chamou de "limitar a memória compartilhada".
//!
//! ## Por que o vendor decide, e por que isso é dito e não escondido
//!
//! **AMD** expõe `amdgpu.gttsize` — é parâmetro de módulo, vai no cmdline do kernel, e
//! funciona. **Intel** não: a memória da iGPU é fixada pelo firmware (DVMT, no setup da UEFI),
//! e o Linux não a altera em runtime. **NVIDIA** discreta não usa GTT desta forma.
//!
//! Uma ferramenta que prometesse o mesmo nos três mentiria em dois. Aqui ela **detecta e diz
//! o que não dá** — §37.48: a mensagem ensina, não culpa nem finge.

use std::path::Path;

/// Quem fabrica a GPU — é o que decide se há knob.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Vendor {
    Amd,
    Intel,
    Nvidia,
    Desconhecido,
}

/// O que se sabe da GPU desta máquina.
#[derive(Debug, Clone, PartialEq)]
pub struct Gpu {
    pub vendor: Vendor,
    /// RAM do sistema que a GPU pode mapear, em bytes. `None` se o driver não expõe.
    pub gtt_bytes: Option<u64>,
    /// VRAM própria da placa, em bytes. `None` em GPU integrada.
    pub vram_bytes: Option<u64>,
}

/// O veredito sobre limitar a memória compartilhada NESTA máquina.
#[derive(Debug, Clone, PartialEq)]
pub enum Limitavel {
    /// Dá, e é este o parâmetro de kernel.
    Sim { parametro: &'static str, atual_mib: u64, sugerido_mib: u64 },
    /// Não dá, e este é o motivo — escrito para a pessoa entender, não para se defender.
    /// CHAVE de catálogo com o motivo (não prosa) — ver o ADR-0014 (D8). A camada `cli`
    /// traduz na hora de imprimir; guardar o texto aqui fazia o relatório em inglês sair com
    /// um parágrafo em português no meio.
    Nao(String),
}

/// **O quê:** lê o vendor a partir do driver carregado em `/sys/class/drm`.
///
/// **Onde:** [`detectar`]. Recebe a raiz como parâmetro para o teste poder montar um `/sys`
/// de mentira — sem isso, nada aqui seria testável fora da máquina de quem escreveu.
pub fn vendor_de(raiz: &Path) -> Vendor {
    // `vendor` do PCI: 0x1002 = AMD/ATI, 0x8086 = Intel, 0x10de = NVIDIA.
    for card in ["card0", "card1", "card2"] {
        let p = raiz.join("class/drm").join(card).join("device/vendor");
        if let Ok(s) = std::fs::read_to_string(&p) {
            return match s.trim().to_lowercase().as_str() {
                "0x1002" => Vendor::Amd,
                "0x8086" => Vendor::Intel,
                "0x10de" => Vendor::Nvidia,
                _ => Vendor::Desconhecido,
            };
        }
    }
    Vendor::Desconhecido
}

/// **O quê:** lê um inteiro de um arquivo do `/sys`. `None` se não existe ou não é número.
fn ler_u64(p: &Path) -> Option<u64> {
    std::fs::read_to_string(p).ok()?.trim().parse().ok()
}

/// **O quê:** o retrato da GPU, a partir de uma raiz de `/sys`.
/// **Onde:** `optimizer diag`; a raiz é parâmetro para o teste.
pub fn detectar_em(raiz: &Path) -> Gpu {
    let vendor = vendor_de(raiz);
    let mut gtt = None;
    let mut vram = None;
    for card in ["card0", "card1", "card2"] {
        let d = raiz.join("class/drm").join(card).join("device");
        if gtt.is_none() {
            gtt = ler_u64(&d.join("mem_info_gtt_total"));
        }
        if vram.is_none() {
            vram = ler_u64(&d.join("mem_info_vram_total"));
        }
    }
    Gpu { vendor, gtt_bytes: gtt, vram_bytes: vram }
}

/// **O quê:** o retrato da GPU desta máquina.
pub fn detectar() -> Gpu {
    detectar_em(Path::new("/sys"))
}

/// **O quê:** o teto sugerido para o GTT, em MiB, dado o que a máquina tem.
///
/// **Onde:** [`avaliar`]. Função PURA — a regra é testável sem hardware.
///
/// **A regra:** um quarto da RAM, com piso de 2 GiB e teto de 8 GiB. O padrão do `amdgpu` é
/// *metade*, e é ele que tira a previsibilidade; um quarto deixa folga para a GPU sem deixar
/// a máquina refém dela. O piso existe porque abaixo de 2 GiB um compositor moderno começa a
/// engasgar, e trocar travamento por travamento não é otimizar.
pub fn sugerir_gtt_mib(ram_bytes: u64) -> u64 {
    let quarto = ram_bytes / 4 / (1024 * 1024);
    quarto.clamp(2048, 8192)
}

/// **O quê:** esta máquina aceita limitar a memória compartilhada? Se sim, com qual valor?
///
/// **Onde:** `optimizer diag` e a sugestão. Função PURA sobre o retrato — nenhum I/O.
pub fn avaliar(g: &Gpu, ram_bytes: u64) -> Limitavel {
    match g.vendor {
        Vendor::Amd => {
            let Some(gtt) = g.gtt_bytes else {
                return Limitavel::Nao("gpu.amd_no_gtt".into());
            };
            Limitavel::Sim {
                parametro: "amdgpu.gttsize",
                atual_mib: gtt / (1024 * 1024),
                sugerido_mib: sugerir_gtt_mib(ram_bytes),
            }
        }
        Vendor::Intel => Limitavel::Nao("gpu.intel_firmware".into()),
        Vendor::Nvidia => Limitavel::Nao("gpu.nvidia_own_vram".into()),
        Vendor::Desconhecido => Limitavel::Nao("gpu.unknown_vendor".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Monta um `/sys` de mentira. É o que torna esta detecção testável fora da máquina de
    /// quem a escreveu — sem isso, o teste seria "funciona no meu computador".
    fn sys_falso(
        nome: &str,
        vendor: &str,
        gtt: Option<u64>,
        vram: Option<u64>,
    ) -> std::path::PathBuf {
        let raiz = std::env::temp_dir().join(format!("opt-gpu-{nome}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&raiz);
        let d = raiz.join("class/drm/card0/device");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("vendor"), format!("{vendor}\n")).unwrap();
        if let Some(g) = gtt {
            std::fs::write(d.join("mem_info_gtt_total"), g.to_string()).unwrap();
        }
        if let Some(v) = vram {
            std::fs::write(d.join("mem_info_vram_total"), v.to_string()).unwrap();
        }
        raiz
    }

    const GIB: u64 = 1024 * 1024 * 1024;

    /// O caso desta máquina: AMD, GTT em metade da RAM.
    #[test]
    fn amd_e_limitavel_e_diz_o_parametro() {
        let raiz = sys_falso("amd", "0x1002", Some(78 * GIB / 10), Some(8 * GIB));
        let g = detectar_em(&raiz);
        assert_eq!(g.vendor, Vendor::Amd);
        match avaliar(&g, 155 * GIB / 10) {
            Limitavel::Sim { parametro, atual_mib, sugerido_mib } => {
                assert_eq!(parametro, "amdgpu.gttsize");
                assert!(atual_mib > 7000, "o atual tem de vir do /sys: {atual_mib}");
                assert!(sugerido_mib < atual_mib, "sugerir mais que o atual não otimiza nada");
            }
            Limitavel::Nao(m) => panic!("AMD devia ser limitável: {m}"),
        }
        let _ = std::fs::remove_dir_all(&raiz);
    }

    /// **O caso que a honestidade exige:** no Intel NÃO dá, e a mensagem diz onde se muda.
    /// Prometer o mesmo nos dois seria mentir em metade das máquinas.
    #[test]
    fn intel_diz_que_nao_da_e_onde_mudar() {
        let raiz = sys_falso("intel", "0x8086", Some(4 * GIB), None);
        let g = detectar_em(&raiz);
        assert_eq!(g.vendor, Vendor::Intel);
        match avaliar(&g, 16 * GIB) {
            Limitavel::Nao(m) => {
                assert!(m.contains("UEFI") || m.contains("firmware"), "tem de dizer ONDE: {m}");
            }
            Limitavel::Sim { .. } => panic!("Intel não tem esse knob — prometer seria mentir"),
        }
        let _ = std::fs::remove_dir_all(&raiz);
    }

    /// Vendor desconhecido NÃO gera sugestão. Chutar parâmetro de kernel é o modo de falha
    /// mais caro que este app tem.
    #[test]
    fn vendor_desconhecido_nao_sugere_nada() {
        let raiz = sys_falso("outro", "0xbeef", Some(GIB), None);
        let g = detectar_em(&raiz);
        assert_eq!(g.vendor, Vendor::Desconhecido);
        assert!(matches!(avaliar(&g, 16 * GIB), Limitavel::Nao(_)));
        let _ = std::fs::remove_dir_all(&raiz);
    }

    /// AMD sem o GTT em `/sys` não vira palpite: sem o valor atual, não há sugestão honesta.
    #[test]
    fn amd_sem_gtt_recusa_em_vez_de_chutar() {
        let raiz = sys_falso("amdsemgtt", "0x1002", None, Some(8 * GIB));
        let g = detectar_em(&raiz);
        assert!(matches!(avaliar(&g, 16 * GIB), Limitavel::Nao(_)));
        let _ = std::fs::remove_dir_all(&raiz);
    }

    /// A sugestão é um quarto da RAM, presa entre 2 e 8 GiB.
    ///
    /// O piso importa: abaixo de 2 GiB um compositor moderno engasga, e trocar travamento por
    /// travamento não é otimizar.
    #[test]
    fn sugestao_e_um_quarto_com_piso_e_teto() {
        assert_eq!(sugerir_gtt_mib(16 * GIB), 4096, "16 GB → 4 GB, o exemplo do pedido");
        assert_eq!(sugerir_gtt_mib(64 * GIB), 8192, "teto de 8 GiB");
        assert_eq!(sugerir_gtt_mib(4 * GIB), 2048, "piso de 2 GiB — abaixo disso o desktop sofre");
        assert_eq!(sugerir_gtt_mib(32 * GIB), 8192);
    }

    /// A sugestão nunca é MAIOR que o atual — sugerir subir o teto não otimiza nada, e
    /// pareceria conselho.
    #[test]
    fn nunca_sugere_mais_do_que_ja_ha() {
        // Máquina com 16 GB: amdgpu põe GTT em 8 GB; a sugestão é 4 GB.
        let g = Gpu { vendor: Vendor::Amd, gtt_bytes: Some(8 * GIB), vram_bytes: Some(8 * GIB) };
        match avaliar(&g, 16 * GIB) {
            Limitavel::Sim { atual_mib, sugerido_mib, .. } => {
                assert!(sugerido_mib < atual_mib, "{sugerido_mib} não é menos que {atual_mib}");
            }
            Limitavel::Nao(m) => panic!("{m}"),
        }
    }
}
