//! SERVIÇOS — o que sobe no boot, quanto custa, e o pouco que dá para desligar.
//!
//! **O quê:** cruza os serviços habilitados com o custo real de boot (`systemd-analyze
//! blame`) e com uma allowlist **curta e justificada item a item**.
//!
//! **Onde:** `optimizer servicos`. Nunca aplica sozinho — o ADR-0011 reserva o `--aplicar`
//! automático às caixas, porque desabilitar serviço é onde se quebra máquina e a causa fica
//! longe do sintoma ("fiquei sem áudio" três dias depois).
//!
//! ## Por que a allowlist é CURTA, e por que isso é a entrega
//!
//! A tentação é uma lista longa de "serviços inúteis" copiada de fórum. Nesta máquina, dos
//! 25 habilitados, quase todos **fazem alguma coisa**: `smartd` avisa que o disco está
//! morrendo, `mcelog` registra erro de hardware, `firewalld` é o firewall, `nordvpnd` é uma
//! VPN que a pessoa instalou de propósito.
//!
//! Desabilitar qualquer um deles "para otimizar" troca alguns megabytes por um risco que
//! ninguém vai ligar à causa. Então a lista tem **dois** itens, e cada um traz o porquê. Uma
//! lista curta que se pode defender vale mais que uma longa que ninguém auditou.
//!
//! ## Medir antes de sugerir
//!
//! Cada item vem com o custo REAL de boot desta máquina, lido do `systemd-analyze`. Sugerir
//! por folclore — "todo mundo desliga isso" — é como se quebra máquina alheia com boa
//! intenção. Se o serviço não custa nada aqui, a ferramenta diz isso e não insiste.

use std::collections::HashMap;

/// Um serviço que a allowlist considera seguro desabilitar, com o porquê.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Seguro {
    pub unidade: &'static str,
    /// O que ele faz — para a pessoa decidir, não para se convencer.
    pub o_que_faz: &'static str,
    /// Por que dá para desligar numa máquina de dev.
    pub por_que_da: &'static str,
    /// O que se perde. **Sempre preenchido**: um item sem custo declarado é um item que
    /// ninguém auditou.
    pub o_que_se_perde: &'static str,
}

/// A allowlist. **Dois itens**, e a brevidade é deliberada — ver o topo do módulo.
pub const SEGUROS: &[Seguro] = &[
    Seguro {
        unidade: "NetworkManager-wait-online.service",
        o_que_faz: "segura o boot até a rede estar de pé",
        por_que_da: "num desktop nada precisa de rede ANTES do login — quem precisa é \
                     servidor com montagem de rede ou serviço que sobe junto. O desktop \
                     conecta em segundo plano e o login não espera",
        o_que_se_perde: "se você tem montagem NFS/SMB em /etc/fstab ou um serviço que exige \
                         rede no boot, ele pode falhar na primeira tentativa",
    },
    Seguro {
        unidade: "appstream-sync-cache.service",
        o_que_faz: "atualiza o cache de metadados da loja de aplicativos",
        por_que_da: "é para a vitrine gráfica de programas. Quem instala por `zypper`/`apt` \
                     no terminal nunca lê esse cache",
        o_que_se_perde: "a loja de aplicativos pode mostrar catálogo desatualizado até você \
                         atualizá-la manualmente",
    },
];

/// Um serviço habilitado, com o que se sabe dele.
#[derive(Debug, Clone, PartialEq)]
pub struct Servico {
    pub unidade: String,
    /// Segundos que ele custou no último boot. `None` = não apareceu no `blame`.
    pub custo_s: Option<f64>,
    /// Está na allowlist?
    pub seguro: Option<&'static Seguro>,
}

/// **O quê:** extrai as unidades habilitadas da saída de
/// `systemctl list-unit-files --state=enabled --type=service --no-legend`.
///
/// **Onde:** [`levantar`]. Função PURA — testável sem systemd.
pub fn parse_habilitados(txt: &str) -> Vec<String> {
    txt.lines()
        .filter_map(|l| l.split_whitespace().next())
        .filter(|u| u.ends_with(".service"))
        // Unidades-modelo (`getty@.service`) não se desabilitam como as outras: são gabaritos
        // de instância. Tratá-las como serviço comum daria erro que não ensina nada.
        .filter(|u| !u.contains('@'))
        .map(str::to_string)
        .collect()
}

/// **O quê:** extrai `unidade → segundos` da saída de `systemd-analyze blame`.
///
/// **Onde:** [`levantar`]. Função PURA.
///
/// O formato é `<tempo> <unidade>`, e o tempo vem em `ms`, `s`, ou `1min 2.345s`. Ignorar o
/// formato composto faria o item mais caro do boot aparecer como o mais barato.
pub fn parse_blame(txt: &str) -> HashMap<String, f64> {
    let mut m = HashMap::new();
    for l in txt.lines() {
        let l = l.trim();
        let Some(pos) = l.rfind(' ') else { continue };
        let (tempo, unidade) = (&l[..pos], l[pos + 1..].trim());
        if unidade.is_empty() {
            continue;
        }
        let mut segundos = 0.0;
        for parte in tempo.split_whitespace() {
            if let Some(v) = parte.strip_suffix("ms") {
                segundos += v.parse::<f64>().unwrap_or(0.0) / 1000.0;
            } else if let Some(v) = parte.strip_suffix("min") {
                segundos += v.parse::<f64>().unwrap_or(0.0) * 60.0;
            } else if let Some(v) = parte.strip_suffix('s') {
                segundos += v.parse::<f64>().unwrap_or(0.0);
            }
        }
        if segundos > 0.0 {
            m.insert(unidade.to_string(), segundos);
        }
    }
    m
}

/// **O quê:** acha o item da allowlist para uma unidade. **Onde:** [`cruzar`].
pub fn seguro_de(unidade: &str) -> Option<&'static Seguro> {
    SEGUROS.iter().find(|s| s.unidade == unidade)
}

/// **O quê:** junta habilitados + custo + allowlist. Função PURA.
///
/// **Onde:** `optimizer servicos`. Ordena por custo decrescente: o que mais pesa aparece
/// primeiro, porque é sobre ele que vale decidir.
pub fn cruzar(habilitados: &[String], custos: &HashMap<String, f64>) -> Vec<Servico> {
    let mut v: Vec<Servico> = habilitados
        .iter()
        .map(|u| Servico {
            unidade: u.clone(),
            custo_s: custos.get(u).copied(),
            seguro: seguro_de(u),
        })
        .collect();
    v.sort_by(|a, b| b.custo_s.unwrap_or(0.0).partial_cmp(&a.custo_s.unwrap_or(0.0)).unwrap());
    v
}

/// **O quê:** vale a pena mexer neste serviço?
///
/// **Onde:** o relatório. Um item da allowlist que **não custa nada nesta máquina** não é
/// recomendado: sugerir mexer no sistema por zero ganho é o oposto de otimizar, e gasta a
/// confiança que a ferramenta vai precisar quando a sugestão importar.
pub fn vale_a_pena(s: &Servico) -> bool {
    s.seguro.is_some() && s.custo_s.unwrap_or(0.0) >= 0.5
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_habilitados_pega_so_service_e_ignora_gabarito() {
        let txt = "\
NetworkManager-wait-online.service enabled enabled
getty@.service                     enabled enabled
foo.timer                          enabled enabled
appstream-sync-cache.service       enabled enabled";
        let v = parse_habilitados(txt);
        assert_eq!(v, vec!["NetworkManager-wait-online.service", "appstream-sync-cache.service"]);
    }

    /// **O formato composto importa.** `1min 2.345s` lido como "1" faria o item mais caro do
    /// boot aparecer como o mais barato — e a ferramenta recomendaria o alvo errado.
    #[test]
    fn parse_blame_entende_ms_s_e_min() {
        let txt = "\
5.136s NetworkManager-wait-online.service
250ms foo.service
1min 2.345s lento.service";
        let m = parse_blame(txt);
        assert!((m["NetworkManager-wait-online.service"] - 5.136).abs() < 0.001);
        assert!((m["foo.service"] - 0.25).abs() < 0.001);
        assert!((m["lento.service"] - 62.345).abs() < 0.01, "composto: {:?}", m["lento.service"]);
    }

    /// Linha malformada não derruba a leitura — a saída vem de outro programa.
    #[test]
    fn parse_blame_ignora_lixo() {
        let m = parse_blame("\n\nlixo sem tempo\n=== cabeçalho ===\n3s ok.service\n");
        assert_eq!(m.len(), 1);
        assert!(m.contains_key("ok.service"));
    }

    /// O caro vem primeiro: é sobre ele que vale decidir.
    #[test]
    fn ordena_por_custo_decrescente() {
        let hab = vec!["a.service".to_string(), "b.service".to_string(), "c.service".to_string()];
        let mut c = HashMap::new();
        c.insert("a.service".to_string(), 0.1);
        c.insert("b.service".to_string(), 5.0);
        let v = cruzar(&hab, &c);
        assert_eq!(v[0].unidade, "b.service");
        assert_eq!(v[2].custo_s, None, "sem custo conhecido vai pro fim");
    }

    /// **A regra que impede recomendação vazia:** item da allowlist que não custa nada AQUI
    /// não é recomendado. Sugerir mexer no sistema por zero ganho gasta a confiança que a
    /// ferramenta vai precisar quando a sugestão importar.
    #[test]
    fn nao_recomenda_o_que_nao_custa_nada_nesta_maquina() {
        let caro = Servico {
            unidade: "NetworkManager-wait-online.service".into(),
            custo_s: Some(5.1),
            seguro: seguro_de("NetworkManager-wait-online.service"),
        };
        assert!(vale_a_pena(&caro));

        let barato = Servico { custo_s: Some(0.01), ..caro.clone() };
        assert!(!vale_a_pena(&barato), "0,01s de ganho não justifica mexer no sistema");

        let fora = Servico { unidade: "smartd.service".into(), custo_s: Some(9.0), seguro: None };
        assert!(!vale_a_pena(&fora), "caro NÃO basta: tem de estar na allowlist");
    }

    /// **A allowlist é curta e cada item se defende.** Um item sem "o que se perde" é um item
    /// que ninguém auditou — e é assim que uma lista de otimização vira uma lista de estragos.
    #[test]
    fn todo_item_da_allowlist_declara_o_que_se_perde() {
        assert!(SEGUROS.len() <= 5, "lista longa é lista que ninguém auditou: {}", SEGUROS.len());
        for s in SEGUROS {
            assert!(s.unidade.ends_with(".service"), "{}", s.unidade);
            assert!(s.o_que_faz.len() > 20, "{}: o que faz é vago demais", s.unidade);
            assert!(s.por_que_da.len() > 40, "{}: a justificativa é fraca", s.unidade);
            assert!(
                s.o_que_se_perde.len() > 30,
                "{}: sem declarar o custo, o item não foi auditado",
                s.unidade
            );
        }
    }

    /// **O que NÃO pode entrar na lista, nunca.** Estes protegem hardware, rede ou dados, e
    /// desligá-los troca megabytes por risco que ninguém liga à causa.
    #[test]
    fn a_allowlist_nao_contem_o_que_protege_a_maquina() {
        for proibido in [
            "smartd.service",         // avisa que o disco está morrendo
            "mcelog.service",         // registra erro de hardware
            "firewalld.service",      // é o firewall
            "auditd.service",         // trilha de auditoria
            "NetworkManager.service", // a rede em si (≠ o wait-online)
            "systemd-journald.service",
        ] {
            assert!(
                seguro_de(proibido).is_none(),
                "{proibido} JAMAIS pode ser recomendado para desabilitar"
            );
        }
    }
}
