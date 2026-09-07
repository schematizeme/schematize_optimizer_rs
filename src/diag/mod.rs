//! DIAGNÓSTICO — o que a máquina tem, e o que dá pra melhorar. **Só leitura.**
//!
//! **O quê:** [`gpu`] mede a memória que a GPU pode tomar do sistema; [`maquina`] o resto.
//!
//! **Onde:** `optimizer diag`, que é o comando padrão. Nada aqui escreve nada: é o piso do
//! ADR-0011 — sugerir é o padrão, aplicar é explícito.

pub mod gpu;
pub mod maquina;
