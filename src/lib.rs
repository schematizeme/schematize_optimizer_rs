//! **schematize optimizer** — deixa a máquina de dev previsível.
//!
//! **O quê:** [`diag`] mede (só leitura); [`caixas`] põe build, navegador e container cada um
//! no seu teto de recurso.
//!
//! **Onde:** app próprio (ADR-0011), derivado do `disco` do schematize — que já resolvia um
//! problema da mesma família, com o padrão *inventariar → mostrar → agir sob confirmação*.
//!
//! ## O piso que este app tem e os outros não precisaram
//!
//! O `disco` apaga **lixo recriável**: no pior caso um build recompila. O Deployer mexe em
//! **credencial**: no pior caso alguém rotaciona um token. Este mexe na **máquina**, e no pior
//! caso ela não liga.
//!
//! Daí as regras, que valem em cada linha deste crate:
//!
//! 1. **Sugerir é o padrão; aplicar é explícito.**
//! 2. **Nenhuma mudança sem o desfazer possível ANTES de aplicá-la** — e o que não se sabe
//!    desfazer não se toca. É por isso que as caixas se recusam a sobrescrever um `.slice`
//!    que não geraram: o `--revert` não saberia o que havia antes.
//! 3. **Detectar antes de propor.** Nada de sugerir `amdgpu.gttsize` num Intel, onde o knob
//!    não existe — a ferramenta diz o que NÃO dá em vez de fingir.
//! 4. **Nada de heurística sobre o que a pessoa usa.** Desabilitar bluetooth "porque parece
//!    inútil" numa máquina de teclado bluetooth deixa alguém sem teclado.

pub mod caixas;
pub mod diag;
pub mod nucleo;
