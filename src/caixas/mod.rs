//! CAIXAS — cada software no seu teto de recurso, via cgroups v2.
//!
//! **O quê:** [`slice`] gera e desfaz os *slices* de usuário do systemd.
//!
//! **Onde:** `optimizer limits`. É a única área que o `--aplicar` executa sozinho (ADR-0011),
//! porque é a única cujo pior caso é "um build ficou mais lento".

pub mod slice;
