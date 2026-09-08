//! NÚCLEO — a infraestrutura mínima para o Optimizer andar sozinho.
//!
//! **O quê:** caminhos e `$HOME` ([`util`]) e execução de processo ([`util::run`]).
//!
//! **Onde:** `diag` e `caixas`.
//!
//! **Por que é cópia:** mesmo motivo do Deployer (ADR-0010) — depender do crate do schematize
//! amarraria os dois apps e mataria a propriedade que justifica este repo: instalar e
//! funcionar sozinho. São ~300 linhas de plataforma, e nada de domínio entra aqui.

pub mod bin;

pub mod desktop;
pub mod i18n;
pub mod icone;
pub mod util;
