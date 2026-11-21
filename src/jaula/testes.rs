//! Testes PUROS da jaula: mapeamento veredito → saída, pedidos hostis, estimativa, diag.

use super::*;

const TETO: u64 = 512;

/// Concluiu vira 0 e não tem mensagem.
#[test]
fn concluiu_sai_com_zero() {
    let d = desfecho(&Veredito::Concluiu(0), TETO, None);
    assert_eq!((d.codigo, d.status, d.chave), (0, "completed", None));
}

/// Falhou repassa o código do filho; 0 nunca mascara falha.
#[test]
fn falhou_repassa_o_codigo_do_filho() {
    assert_eq!(desfecho(&Veredito::Falhou(42), TETO, None).codigo, 42);
    assert_eq!(desfecho(&Veredito::Falhou(127), TETO, None).codigo, 127);
    assert_eq!(desfecho(&Veredito::Falhou(0), TETO, None).codigo, 1);
}

/// OOM é 137 com o teto na mensagem e o pico no resultado.
#[test]
fn oom_e_137_com_teto_e_pico() {
    let v = Veredito::EstourouMemoria { pico_mib: Some(498), teto_mib: 500 };
    let d = desfecho(&v, 999, None);
    assert_eq!((d.codigo, d.status, d.teto_mib, d.pico_mib), (137, "oom_killed", 500, Some(498)));
    assert_eq!(d.chave, Some("jaula.rodar.oom"));
    assert_eq!(d.args, vec![("teto", "500".to_string())]);
}

/// Timeout é 124; sem backend é 2 e não confunde com os outros.
#[test]
fn timeout_124_e_sem_backend_2() {
    let t = desfecho(&Veredito::Timeout, TETO, Some(7));
    assert_eq!((t.codigo, t.status), (124, "timeout"));
    assert_eq!(t.args, vec![("s", "7".to_string())]);
    let s = desfecho(&Veredito::SemBackend("x".into()), TETO, None);
    assert_eq!((s.codigo, s.status, s.chave), (2, "no_backend", Some("jaula.rodar.sem_backend")));
}

/// Nenhum veredito não-OOM usa o código reservado 137 por conta própria.
#[test]
fn so_o_oom_usa_137_e_so_o_timeout_usa_124() {
    for v in [Veredito::Concluiu(0), Veredito::Timeout, Veredito::SemBackend(String::new())] {
        assert_ne!(desfecho(&v, TETO, None).codigo, 137);
    }
    assert_ne!(desfecho(&Veredito::Falhou(1), TETO, None).codigo, 124);
}

/// Pedido vazio: classe média (1024 MiB), sem timeout.
#[test]
fn pedido_vazio_e_medio() {
    let o = montar_orcamento(&Pedido::default()).unwrap();
    assert_eq!((o.mem_max_mib, o.timeout), (1024, None));
}

/// Cada classe mapeia para o teto da lib.
#[test]
fn classes_mapeiam_para_o_teto() {
    for (c, m) in [("leve", 512), ("medio", 1024), ("pesado", 2048)] {
        let o = montar_orcamento(&Pedido { classe: Some(c), ..Default::default() }).unwrap();
        assert_eq!(o.mem_max_mib, m);
    }
}

/// Pedidos hostis são recusados com o motivo certo, nunca viram teto errado.
#[test]
fn pedidos_hostis_sao_recusados() {
    let p = |classe, mem_mib, timeout_s, adr| Pedido { classe, mem_mib, timeout_s, adr };
    let casos = [
        (p(Some("gigante"), None, None, None), ErroPedido::ClasseInvalida("gigante".into())),
        (p(Some(""), None, None, None), ErroPedido::ClasseInvalida(String::new())),
        (p(Some("leve"), Some(600), None, None), ErroPedido::ClasseEMem),
        (p(None, Some(0), None, None), ErroPedido::MemZero),
        (p(None, Some(2049), None, None), ErroPedido::AcimaDoTeto(2049)),
        (p(None, Some(u64::MAX), None, None), ErroPedido::AcimaDoTeto(u64::MAX)),
        (p(None, Some(4096), None, Some("  ")), ErroPedido::AdrVazio),
        (p(None, None, None, Some("ADR-1")), ErroPedido::AdrSemMem),
        (p(None, Some(600), Some(0), None), ErroPedido::TimeoutZero),
    ];
    for (pedido, esperado) in casos {
        assert_eq!(montar_orcamento(&pedido).unwrap_err(), esperado);
    }
}

/// Acima de 2048 só passa com ADR; abaixo de 500 sobe para o piso; timeout entra no orçamento.
#[test]
fn adr_libera_acima_e_piso_sobe() {
    let p = Pedido { mem_mib: Some(4096), adr: Some("ADR-0042"), ..Default::default() };
    assert_eq!(montar_orcamento(&p).unwrap().mem_max_mib, 4096);
    let p = Pedido { mem_mib: Some(100), timeout_s: Some(30), ..Default::default() };
    let o = montar_orcamento(&p).unwrap();
    assert_eq!((o.mem_max_mib, o.timeout), (500, Some(Duration::from_secs(30))));
}

/// Toda recusa tem chave de mensagem própria (catálogo), sem texto cru.
#[test]
fn toda_recusa_tem_chave_i18n() {
    let erros = [
        ErroPedido::ClasseInvalida("x".into()),
        ErroPedido::ClasseEMem,
        ErroPedido::MemZero,
        ErroPedido::TimeoutZero,
        ErroPedido::AdrSemMem,
        ErroPedido::AcimaDoTeto(3000),
        ErroPedido::AdrVazio,
    ];
    for e in erros {
        assert!(e.mensagem().0.starts_with("jaula.err."), "{e:?}");
    }
}

/// `estimar` sem tipo lista os seis na ordem; com tipo, só o pedido; valores da lib.
#[test]
fn estimar_lista_os_seis_e_filtra() {
    let todas = estimativas(None);
    let tipos: Vec<_> = todas.iter().map(|l| l.tipo).collect();
    assert_eq!(tipos, TIPOS);
    let build = estimativas(tipo_de_slug("build"));
    assert_eq!(build.len(), 1);
    assert_eq!(
        (build[0].classe, build[0].mem_max_mib, build[0].mem_high_mib),
        ("pesado", 2048, 1740)
    );
    assert_eq!(build[0].jobs, 2);
    let script = &estimativas(tipo_de_slug("script"))[0];
    assert_eq!((script.classe, script.mem_max_mib, script.jobs), ("leve", 512, 1));
}

/// Os slugs de tipo fazem ida e volta; o desconhecido é `None`.
#[test]
fn slugs_de_tipo_fazem_ida_e_volta() {
    for s in TIPOS {
        assert_eq!(tipo_de_slug(s).map(slug_tipo), Some(s));
    }
    assert!(tipo_de_slug("AGENTE").is_none() && tipo_de_slug("").is_none());
}

/// O wrapper do modo JSON leva o argv do usuário intacto, depois do script fixo.
#[test]
fn argv_json_nao_interpola_nada() {
    let v = argv_para_json(&["echo".into(), "$(rm -rf /)".into()]);
    assert_eq!(&v[..4], ["sh", "-c", r#"exec "$@" 1>&2"#, "jaula"]);
    assert_eq!(&v[4..], ["echo", "$(rm -rf /)"]);
}

/// `faltas` aponta só o que falta; tudo presente não falta nada (prlimit não é requisito).
#[test]
fn faltas_aponta_so_o_que_falta() {
    let ok = Fatos {
        cgroup_v2: true,
        controller_memory: true,
        systemd_run: true,
        bus_usuario: true,
        prlimit: false,
    };
    assert!(faltas(&ok).is_empty());
    let ruim = Fatos { controller_memory: false, systemd_run: false, ..ok };
    assert_eq!(faltas(&ruim), ["memory_controller", "systemd_run"]);
}
