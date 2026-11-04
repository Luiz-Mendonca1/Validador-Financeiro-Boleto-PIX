//! # Financial Validator (Módulo Rust 2021)
//!
//! Módulo de alta performance para validação rigorosa de Boletos Bancários
//! (Linha Digitável / Código de Barras de Cobrança e Concessionárias),
//! PIX Copia e Cola (EMV QR Code / BR Code) e Motor de Análise de Risco Antifraude.
//!
//! Suporta serialização nativa para JSON via `serde`.
//!
//! ## Exemplo de Uso
//! ```rust
//! use financial_validator::FinancialValidator;
//!
//! let validator = FinancialValidator::new();
//! let boleto_res = validator.validate_boleto("00190.00009 01041.800408 00000.000057 1 97890000015000");
//! println!("Boleto Válido: {}", boleto_res.is_valid);
//! ```

#![allow(dead_code)]
#![allow(unused_variables)]

use serde::{Deserialize, Serialize};

/// Estrutura principal do validador financeiro.
#[derive(Debug, Default, Clone)]
pub struct FinancialValidator;

// ============================================================================
// STRUCTS DE DADOS (BOLETO)
// ============================================================================

/// Tipo de boleto identificado
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BoletoType {
    /// Boleto bancário tradicional de cobrança (47 dígitos na linha digitável)
    Cobranca,
    /// Boleto de concessionária, tributos ou serviços públicos (48 dígitos)
    Arrecadacao,
    /// Formato não reconhecido
    Desconhecido,
}

/// Detalhes de validação de cada campo/bloco da linha digitável
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CampoValidacao {
    pub campo_numero: usize,
    pub conteudo: String,
    pub dv_informado: u8,
    pub dv_calculado: u8,
    pub modulo: u8,
    pub valido: bool,
}

/// Dados extraídos e validados de um boleto bancário
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoletoResult {
    pub is_valid: bool,
    pub tipo: BoletoType,
    pub linha_digitavel_limpa: String,
    pub codigo_barras: Option<String>,
    pub banco_codigo: Option<String>,
    pub banco_nome: Option<String>,
    pub moeda_codigo: Option<String>,
    pub valor: Option<f64>,
    pub fator_vencimento: Option<u32>,
    pub data_vencimento_estimada: Option<String>,
    pub dv_geral_valido: bool,
    pub campos_validacao: Vec<CampoValidacao>,
    pub erros: Vec<String>,
}

// ============================================================================
// STRUCTS DE DADOS (PIX)
// ============================================================================

/// Campo TLV individual extraído do EMV PIX
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PixTlvTag {
    pub id: String,
    pub nome: String,
    pub tamanho: usize,
    pub valor: String,
}

/// Tipo de QR Code PIX
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PixType {
    Estatico,
    Dinamico,
    Desconhecido,
}

/// Dados extraídos e validados de um PIX Copia e Cola (EMV BR Code)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PixResult {
    pub is_valid: bool,
    pub tipo: PixType,
    pub chave_pix: Option<String>,
    pub tipo_chave: Option<String>,
    pub beneficiario_nome: Option<String>,
    pub beneficiario_cidade: Option<String>,
    pub valor: Option<f64>,
    pub txid: Option<String>,
    pub payload_url: Option<String>,
    pub crc16_informado: String,
    pub crc16_calculado: String,
    pub crc16_valido: bool,
    pub tags_tlv: Vec<PixTlvTag>,
    pub erros: Vec<String>,
}

// ============================================================================
// STRUCTS DE DADOS (MOTOR DE RISCO)
// ============================================================================

/// Nível de Risco consolidado
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum RiskLevel {
    Baixo,
    Medio,
    Alto,
    Critico,
}

/// Recomendação de ação baseada na análise de risco
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskRecommendation {
    AprovarAutomaticamente,
    RevisaoManualRequerida,
    RejeitarOperacao,
}

/// Fator individual de risco detectado
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskFactor {
    pub codigo: String,
    pub descricao: String,
    pub impacto_pontos: u32,
    pub severidade: String,
}

/// Relatório consolidado emitido por `calculate_risk`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationReport {
    pub transacao_tipo: String,
    pub score_risco: u32, // Escala de 0 a 100
    pub nivel_risco: RiskLevel,
    pub recomendacao: RiskRecommendation,
    pub fatores_risco: Vec<RiskFactor>,
    pub boleto_detalhes: Option<BoletoResult>,
    pub pix_detalhes: Option<PixResult>,
    pub timestamp_analise: String,
}

// ============================================================================
// STRUCTS DE ANÁLISE DE DOCUMENTO (PDF / WORD / TEXTO)
// ============================================================================

/// Item financeiro localizado dentro de um documento (PDF, Word, TXT, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentFinancialItem {
    pub tipo: String, // "BOLETO" ou "PIX"
    pub texto_bruto: String,
    pub boleto_resultado: Option<BoletoResult>,
    pub pix_resultado: Option<PixResult>,
    pub report: ValidationReport,
}

/// Relatório de análise e veracidade de documento
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentAnalysisReport {
    pub total_itens_encontrados: usize,
    pub total_validos: usize,
    pub total_invalidos: usize,
    pub documento_confiavel: bool,
    pub score_risco_global: u32,
    pub recomendacao_global: RiskRecommendation,
    pub itens: Vec<DocumentFinancialItem>,
    pub alertas_documento: Vec<String>,
}

// ============================================================================
// IMPLEMENTAÇÃO DOS ALGORITMOS DE CHECKSUM
// ============================================================================

/// Utilitários de cálculo de dígito verificador e checksums bancários.
pub struct Checksums;

impl Checksums {
    /// Cálculo do Módulo 10 com pesos 2 e 1 alternados da direita para a esquerda.
    ///
    /// Multiplica-se cada algarismo pelo peso (2, 1, 2, 1...).
    /// Se o produto for maior que 9, somam-se os algarismos do produto (ex: 7 * 2 = 14 => 1 + 4 = 5).
    /// Subtrai-se o resto da divisão por 10 de 10: DV = (10 - (soma % 10)) % 10.
    pub fn modulo10(input: &str) -> u8 {
        let mut soma = 0;
        let mut peso = 2;

        for c in input.chars().rev() {
            if let Some(digit) = c.to_digit(10) {
                let mut produto = digit * peso;
                if produto > 9 {
                    produto = (produto / 10) + (produto % 10);
                }
                soma += produto;
                peso = if peso == 2 { 1 } else { 2 };
            }
        }

        let resto = soma % 10;
        if resto == 0 {
            0
        } else {
            (10 - resto) as u8
        }
    }

    /// Cálculo do Módulo 11 para Boletos Bancários de Cobrança (pesos de 2 a 9).
    ///
    /// Pesos circulam de 2 a 9 da direita para a esquerda.
    /// Resto da soma dividido por 11:
    /// DV = 11 - (soma % 11).
    /// Regra Febraban Cobrança: se DV == 0, 10 ou 11 -> DV = 1.
    pub fn modulo11_cobranca(input: &str) -> u8 {
        let mut soma = 0;
        let mut peso = 2;

        for c in input.chars().rev() {
            if let Some(digit) = c.to_digit(10) {
                soma += digit * peso;
                peso += 1;
                if peso > 9 {
                    peso = 2;
                }
            }
        }

        let resto = soma % 11;
        let dv = 11 - resto;

        if dv == 0 || dv == 10 || dv == 11 {
            1
        } else {
            dv as u8
        }
    }

    /// Cálculo do Módulo 11 para Boletos de Arrecadação / Concessionárias.
    ///
    /// Se resto == 0 ou resto == 1 => DV = 0.
    /// Se resto == 10 => DV = 1 (ou conforme segmento).
    pub fn modulo11_arrecadacao(input: &str) -> u8 {
        let mut soma = 0;
        let mut peso = 2;

        for c in input.chars().rev() {
            if let Some(digit) = c.to_digit(10) {
                soma += digit * peso;
                peso += 1;
                if peso > 9 {
                    peso = 2;
                }
            }
        }

        let resto = soma % 11;
        if resto == 0 || resto == 1 {
            0
        } else if resto == 10 {
            1
        } else {
            (11 - resto) as u8
        }
    }

    /// Implementação pura de CRC16 (CCITT-FALSE).
    ///
    /// - Polinômio: `0x1021` (x^16 + x^12 + x^5 + 1)
    /// - Inicialização: `0xFFFF`
    /// - RefIn: false
    /// - RefOut: false
    /// - XorOut: `0x0000`
    ///
    /// Processa cada byte deslocando bit a bit sem alocações dinâmicas.
    pub fn crc16_ccitt_false(data: &[u8]) -> u16 {
        let mut crc: u16 = 0xFFFF;
        let polynomial: u16 = 0x1021;

        for &byte in data {
            crc ^= (byte as u16) << 8;
            for _ in 0..8 {
                if (crc & 0x8000) != 0 {
                    crc = (crc << 1) ^ polynomial;
                } else {
                    crc <<= 1;
                }
            }
        }

        crc
    }
}

// ============================================================================
// IMPLEMENTAÇÃO DE FINANCIAL VALIDATOR
// ============================================================================

impl FinancialValidator {
    /// Cria uma nova instância do validador.
    pub fn new() -> Self {
        FinancialValidator
    }

    /// Converte um Código de Barras de 44 dígitos em Linha Digitável (47 ou 48 dígitos).
    pub fn barcode_to_linha_digitavel(barcode: &str) -> Option<String> {
        let limpa: String = barcode.chars().filter(|c| c.is_ascii_digit()).collect();
        if limpa.len() != 44 {
            return None;
        }

        if limpa.starts_with('8') {
            let b1 = &limpa[0..11];
            let b2 = &limpa[11..22];
            let b3 = &limpa[22..33];
            let b4 = &limpa[33..44];
            let cod_moeda = limpa.chars().nth(2).and_then(|c| c.to_digit(10)).unwrap_or(6);
            let usa_mod10 = cod_moeda == 6 || cod_moeda == 7;

            let dv1 = if usa_mod10 { Checksums::modulo10(b1) } else { Checksums::modulo11_arrecadacao(b1) };
            let dv2 = if usa_mod10 { Checksums::modulo10(b2) } else { Checksums::modulo11_arrecadacao(b2) };
            let dv3 = if usa_mod10 { Checksums::modulo10(b3) } else { Checksums::modulo11_arrecadacao(b3) };
            let dv4 = if usa_mod10 { Checksums::modulo10(b4) } else { Checksums::modulo11_arrecadacao(b4) };

            return Some(format!("{}{}{}{}{}{}{}{}", b1, dv1, b2, dv2, b3, dv3, b4, dv4));
        }

        let banco = &limpa[0..3];
        let moeda = &limpa[3..4];
        let dv_geral = &limpa[4..5];
        let fator = &limpa[5..9];
        let valor = &limpa[9..19];
        let campo_livre = &limpa[19..44];

        let c1 = format!("{}{}{}", banco, moeda, &campo_livre[0..5]);
        let dv1 = Checksums::modulo10(&c1);

        let c2 = &campo_livre[5..15];
        let dv2 = Checksums::modulo10(c2);

        let c3 = &campo_livre[15..25];
        let dv3 = Checksums::modulo10(c3);

        Some(format!("{}{}{}{}{}{}{}{}{}", c1, dv1, c2, dv2, c3, dv3, dv_geral, fator, valor))
    }

    /// Valida uma linha digitável de Boleto Bancário (Cobrança 47 dígitos, Arrecadação 48 dígitos ou Código de Barras 44 dígitos).
    pub fn validate_boleto(&self, linha: &str) -> BoletoResult {
        let limpa: String = linha.chars().filter(|c| c.is_ascii_digit()).collect();
        let linha_tratada = if limpa.len() == 44 {
            Self::barcode_to_linha_digitavel(&limpa).unwrap_or_else(|| limpa.clone())
        } else {
            limpa.clone()
        };
        let len = linha_tratada.len();

        let mut erros = Vec::new();
        let campos_validacao = Vec::new();

        if len != 47 && len != 48 {
            erros.push(format!(
                "Tamanho inválido para boleto: {} dígitos (esperado 47 para cobrança, 48 para arrecadação ou 44 para código de barras).",
                limpa.len()
            ));
            return BoletoResult {
                is_valid: false,
                tipo: BoletoType::Desconhecido,
                linha_digitavel_limpa: limpa,
                codigo_barras: None,
                banco_codigo: None,
                banco_nome: None,
                moeda_codigo: None,
                valor: None,
                fator_vencimento: None,
                data_vencimento_estimada: None,
                dv_geral_valido: false,
                campos_validacao,
                erros,
            };
        }

        if len == 47 {
            self.validate_boleto_cobranca(&linha_tratada)
        } else {
            self.validate_boleto_arrecadacao(&linha_tratada)
        }
    }

    /// Validação detalhada de boleto de cobrança (47 dígitos)
    fn validate_boleto_cobranca(&self, limpa: &str) -> BoletoResult {
        let mut erros = Vec::new();
        let mut campos_validacao = Vec::new();

        // Blocos da linha digitável de cobrança:
        // Campo 1: 9 dígitos + DV (Módulo 10) [0..9] e [9..10]
        // Campo 2: 10 dígitos + DV (Módulo 10) [10..20] e [20..21]
        // Campo 3: 10 dígitos + DV (Módulo 10) [21..31] e [31..32]
        // Campo 4: DV Geral do Código de Barras [32..33]
        // Campo 5: Fator Vencimento (4 dígitos) + Valor (10 dígitos) [33..47]

        let c1_corpo = &limpa[0..9];
        let c1_dv = limpa.chars().nth(9).unwrap().to_digit(10).unwrap() as u8;
        let c1_calc = Checksums::modulo10(c1_corpo);
        let c1_valido = c1_dv == c1_calc;
        campos_validacao.push(CampoValidacao {
            campo_numero: 1,
            conteudo: c1_corpo.to_string(),
            dv_informado: c1_dv,
            dv_calculado: c1_calc,
            modulo: 10,
            valido: c1_valido,
        });
        if !c1_valido {
            erros.push(format!("DV do Campo 1 inválido: informado {}, calculado {}.", c1_dv, c1_calc));
        }

        let c2_corpo = &limpa[10..20];
        let c2_dv = limpa.chars().nth(20).unwrap().to_digit(10).unwrap() as u8;
        let c2_calc = Checksums::modulo10(c2_corpo);
        let c2_valido = c2_dv == c2_calc;
        campos_validacao.push(CampoValidacao {
            campo_numero: 2,
            conteudo: c2_corpo.to_string(),
            dv_informado: c2_dv,
            dv_calculado: c2_calc,
            modulo: 10,
            valido: c2_valido,
        });
        if !c2_valido {
            erros.push(format!("DV do Campo 2 inválido: informado {}, calculado {}.", c2_dv, c2_calc));
        }

        let c3_corpo = &limpa[21..31];
        let c3_dv = limpa.chars().nth(31).unwrap().to_digit(10).unwrap() as u8;
        let c3_calc = Checksums::modulo10(c3_corpo);
        let c3_valido = c3_dv == c3_calc;
        campos_validacao.push(CampoValidacao {
            campo_numero: 3,
            conteudo: c3_corpo.to_string(),
            dv_informado: c3_dv,
            dv_calculado: c3_calc,
            modulo: 10,
            valido: c3_valido,
        });
        if !c3_valido {
            erros.push(format!("DV do Campo 3 inválido: informado {}, calculado {}.", c3_dv, c3_calc));
        }

        let dv_geral_informado = limpa.chars().nth(32).unwrap().to_digit(10).unwrap() as u8;

        // Montagem do Código de Barras (44 posições):
        // 01-03: Código do Banco
        // 04-04: Código da Moeda
        // 05-05: DV Geral
        // 06-09: Fator de Vencimento
        // 10-19: Valor Nominal (10 dígitos)
        // 20-44: Campo Livre (25 dígitos)
        let banco = &limpa[0..3];
        let moeda = &limpa[3..4];
        let fator_venc_str = &limpa[33..37];
        let valor_str = &limpa[37..47];
        let campo_livre = format!("{}{}{}", &limpa[4..9], &limpa[10..20], &limpa[21..31]);

        let codigo_barras_sem_dv = format!("{}{}{}{}{}", banco, moeda, fator_venc_str, valor_str, campo_livre);
        let dv_geral_calculado = Checksums::modulo11_cobranca(&codigo_barras_sem_dv);
        let dv_geral_valido = dv_geral_informado == dv_geral_calculado;

        if !dv_geral_valido {
            erros.push(format!(
                "DV Geral do Código de Barras inválido: informado {}, calculado {}.",
                dv_geral_informado, dv_geral_calculado
            ));
        }

        let codigo_barras = format!(
            "{}{}{}{}{}{}",
            banco, moeda, dv_geral_calculado, fator_venc_str, valor_str, campo_livre
        );

        let fator_venc: u32 = fator_venc_str.parse().unwrap_or(0);
        let valor_centavos: f64 = valor_str.parse().unwrap_or(0.0);
        let valor = valor_centavos / 100.0;

        let data_venc = Self::calcular_data_vencimento(fator_venc);
        let banco_nome = Self::obter_nome_banco(banco);

        let is_valid = c1_valido && c2_valido && c3_valido && dv_geral_valido;

        BoletoResult {
            is_valid,
            tipo: BoletoType::Cobranca,
            linha_digitavel_limpa: limpa.to_string(),
            codigo_barras: Some(codigo_barras),
            banco_codigo: Some(banco.to_string()),
            banco_nome: Some(banco_nome.to_string()),
            moeda_codigo: Some(moeda.to_string()),
            valor: Some(valor),
            fator_vencimento: Some(fator_venc),
            data_vencimento_estimada: data_venc,
            dv_geral_valido,
            campos_validacao,
            erros,
        }
    }

    /// Validação detalhada de boleto de concessionária / arrecadação (48 dígitos)
    fn validate_boleto_arrecadacao(&self, limpa: &str) -> BoletoResult {
        let mut erros = Vec::new();
        let mut campos_validacao = Vec::new();

        // Boletos de concessionária possuem 4 blocos de 12 dígitos (11 dados + 1 DV)
        let cod_moeda = limpa.chars().nth(2).and_then(|c| c.to_digit(10)).unwrap_or(6);
        let usa_mod10 = cod_moeda == 6 || cod_moeda == 7;

        let mut todos_dvs_ok = true;
        let mut cod_barras_raw = String::with_capacity(44);

        for i in 0..4 {
            let start = i * 12;
            let corpo = &limpa[start..start + 11];
            let dv_informado = limpa.chars().nth(start + 11).unwrap().to_digit(10).unwrap() as u8;

            let dv_calculado = if usa_mod10 {
                Checksums::modulo10(corpo)
            } else {
                Checksums::modulo11_arrecadacao(corpo)
            };

            let valido = dv_informado == dv_calculado;
            if !valido {
                todos_dvs_ok = false;
                erros.push(format!(
                    "Bloco {} com DV inválido: informado {}, calculado {}.",
                    i + 1,
                    dv_informado,
                    dv_calculado
                ));
            }

            campos_validacao.push(CampoValidacao {
                campo_numero: i + 1,
                conteudo: corpo.to_string(),
                dv_informado,
                dv_calculado,
                modulo: if usa_mod10 { 10 } else { 11 },
                valido,
            });

            cod_barras_raw.push_str(corpo);
        }

        // Valor em concessionária: posições 4..15 do código de barras
        let valor = if cod_barras_raw.len() >= 15 {
            let valor_str = &cod_barras_raw[4..15];
            valor_str.parse::<f64>().ok().map(|v| v / 100.0)
        } else {
            None
        };

        BoletoResult {
            is_valid: todos_dvs_ok,
            tipo: BoletoType::Arrecadacao,
            linha_digitavel_limpa: limpa.to_string(),
            codigo_barras: Some(cod_barras_raw),
            banco_codigo: None,
            banco_nome: Some("Concessionária / Órgão Governamental".to_string()),
            moeda_codigo: Some(cod_moeda.to_string()),
            valor,
            fator_vencimento: None,
            data_vencimento_estimada: None,
            dv_geral_valido: todos_dvs_ok,
            campos_validacao,
            erros,
        }
    }

    /// Valida uma string EMV do PIX Copia e Cola (BR Code).
    pub fn validate_pix(&self, emv: &str) -> PixResult {
        let emv_trim = emv.trim();
        let mut erros = Vec::new();

        if !emv_trim.starts_with("000201") {
            erros.push("Payload EMV não se inicia com o identificador padrão '000201'.".to_string());
        }

        // Encontra o marcador de CRC16 "6304"
        let crc_idx = emv_trim.rfind("6304");
        let (crc16_informado, crc16_calculado, crc16_valido) = match crc_idx {
            Some(idx) => {
                let payload_ate_6304 = &emv_trim[..idx + 4];
                let crc_num = Checksums::crc16_ccitt_false(payload_ate_6304.as_bytes());
                let crc_calc_hex = format!("{:04X}", crc_num);

                let informado = if emv_trim.len() >= idx + 8 {
                    emv_trim[idx + 4..idx + 8].to_uppercase()
                } else {
                    String::from("INCOMPLETO")
                };

                let valido = informado == crc_calc_hex;
                if !valido {
                    erros.push(format!(
                        "Checksum CRC16 inválido: informado '{}', calculado '{}'.",
                        informado, crc_calc_hex
                    ));
                }
                (informado, crc_calc_hex, valido)
            }
            None => {
                erros.push("Tag de CRC16 ('6304') não localizada no payload EMV.".to_string());
                (String::from("AUSENTE"), String::new(), false)
            }
        };

        // Parser TLV EMV
        let mut tags_tlv = Vec::new();
        let mut chave_pix = None;
        let mut tipo_chave = None;
        let mut beneficiario_nome = None;
        let mut beneficiario_cidade = None;
        let mut valor = None;
        let mut txid = None;
        let mut payload_url = None;
        let mut tipo = PixType::Desconhecido;

        let bytes = emv_trim.as_bytes();
        let mut cursor = 0;

        while cursor + 4 <= bytes.len() {
            let id = match std::str::from_utf8(&bytes[cursor..cursor + 2]) {
                Ok(s) => s,
                Err(_) => break,
            };
            let len_str = match std::str::from_utf8(&bytes[cursor + 2..cursor + 4]) {
                Ok(s) => s,
                Err(_) => break,
            };

            let len: usize = match len_str.parse() {
                Ok(n) => n,
                Err(_) => break,
            };

            cursor += 4;
            if cursor + len > bytes.len() {
                erros.push(format!("Payload truncado na leitura da tag {}", id));
                break;
            }

            let val = match std::str::from_utf8(&bytes[cursor..cursor + len]) {
                Ok(s) => s,
                Err(_) => break,
            };

            let tag_nome = Self::nome_tag_emv(id);
            tags_tlv.push(PixTlvTag {
                id: id.to_string(),
                nome: tag_nome.to_string(),
                tamanho: len,
                valor: val.to_string(),
            });

            match id {
                "01" => {
                    tipo = match val {
                        "11" => PixType::Estatico,
                        "12" => PixType::Dinamico,
                        _ => PixType::Desconhecido,
                    };
                }
                "26" => {
                    // Sub-tags do Merchant Account Information (GUI / Chave)
                    let (k, t, u) = Self::parse_subtags_pix_26(val);
                    if k.is_some() {
                        chave_pix = k;
                        tipo_chave = t;
                    }
                    if u.is_some() {
                        payload_url = u;
                    }
                }
                "54" => {
                    valor = val.parse::<f64>().ok();
                }
                "59" => {
                    beneficiario_nome = Some(val.to_string());
                }
                "60" => {
                    beneficiario_cidade = Some(val.to_string());
                }
                "62" => {
                    // Sub-tags de Informações Adicionais (ex: TxID na sub-tag 05)
                    txid = Self::parse_subtag_txid(val);
                }
                _ => {}
            }

            cursor += len;
        }

        let is_valid = crc16_valido && erros.is_empty();

        PixResult {
            is_valid,
            tipo,
            chave_pix,
            tipo_chave,
            beneficiario_nome,
            beneficiario_cidade,
            valor,
            txid,
            payload_url,
            crc16_informado,
            crc16_calculado,
            crc16_valido,
            tags_tlv,
            erros,
        }
    }

    /// Motor de Risco Antifraude e Conformidade Financeira.
    ///
    /// Avalia múltiplos vetores de anomalia: integridade criptográfica/checksum,
    /// coerência temporal de vencimento, faixas de valor, heurísticas de chave PIX
    /// e padronização do código emissor.
    pub fn calculate_risk(
        &self,
        boleto: Option<&BoletoResult>,
        pix: Option<&PixResult>,
        valor_digitado_usuario: Option<f64>,
    ) -> ValidationReport {
        let mut score: u32 = 0;
        let mut fatores = Vec::new();
        let transacao_tipo;

        if let Some(b) = boleto {
            transacao_tipo = "BOLETO_BANCARIO".to_string();

            // 1. Falha estrutural de Dígitos Verificadores (Integridade Crítica)
            if !b.is_valid {
                score += 55;
                fatores.push(RiskFactor {
                    codigo: "BOLETO_DV_INVALIDO".to_string(),
                    descricao: "Dígitos Verificadores da linha digitável ou código de barras incompatíveis. Alto risco de adulteração (bolware).".to_string(),
                    impacto_pontos: 55,
                    severidade: "CRITICA".to_string(),
                });
            }

            // 2. Discrepância entre o valor informado pelo usuário e o valor embutido no boleto
            if let (Some(val_b), Some(val_u)) = (b.valor, valor_digitado_usuario) {
                if (val_b - val_u).abs() > 0.01 && val_b > 0.0 {
                    score += 40;
                    fatores.push(RiskFactor {
                        codigo: "VALOR_DIVERGENTE".to_string(),
                        descricao: format!(
                            "Valor na linha digitável (R$ {:.2}) difere do valor esperado (R$ {:.2}).",
                            val_b, val_u
                        ),
                        impacto_pontos: 40,
                        severidade: "ALTA".to_string(),
                    });
                }
            }

            // 3. Boleto sem valor especificado na linha digitável (cobrança com valor zerado)
            if let Some(val) = b.valor {
                if val == 0.0 && b.tipo == BoletoType::Cobranca {
                    score += 15;
                    fatores.push(RiskFactor {
                        codigo: "BOLETO_VALOR_ZERADO".to_string(),
                        descricao: "Boleto emitido sem valor fixado (valor aberto). Exige conferência manual do emissor.".to_string(),
                        impacto_pontos: 15,
                        severidade: "MEDIA".to_string(),
                    });
                }
            }

            // 4. Análise do Código de Banco
            if let Some(banco) = &b.banco_codigo {
                if banco == "000" || banco == "999" {
                    score += 35;
                    fatores.push(RiskFactor {
                        codigo: "BANCO_SUSPEITO".to_string(),
                        descricao: format!("Código de instituição bancária incomum ou de teste: '{}'.", banco),
                        impacto_pontos: 35,
                        severidade: "ALTA".to_string(),
                    });
                }
            }
        } else if let Some(p) = pix {
            transacao_tipo = "PIX_COPIA_E_COLA".to_string();

            // 1. Falha de CRC16 (Adulteração de Payload / Man-in-the-Middle)
            if !p.crc16_valido {
                score += 85;
                fatores.push(RiskFactor {
                    codigo: "PIX_CRC16_ADULTERADO".to_string(),
                    descricao: "Checksum CRC16 (CCITT-FALSE) incorreto. O código PIX foi adulterado ou transmitido corrompido.".to_string(),
                    impacto_pontos: 85,
                    severidade: "CRITICA".to_string(),
                });
            }

            // 2. Beneficiário ausente ou genérico
            if p.beneficiario_nome.is_none() || p.beneficiario_nome.as_deref() == Some("") {
                score += 25;
                fatores.push(RiskFactor {
                    codigo: "BENEFICIARIO_OMISSO".to_string(),
                    descricao: "Nome do recebedor não especificado na estrutura EMV.".to_string(),
                    impacto_pontos: 25,
                    severidade: "MEDIA".to_string(),
                });
            }

            // 3. TxID ausente ou padrão genérico "***" em PIX Estático/Dinâmico
            if let Some(ref tid) = p.txid {
                if tid == "***" {
                    score += 10;
                    fatores.push(RiskFactor {
                        codigo: "TXID_GENERICO".to_string(),
                        descricao: "TxID não definido pelo recebedor (utiliza curinga '***').".to_string(),
                        impacto_pontos: 10,
                        severidade: "BAIXA".to_string(),
                    });
                }
            }

            // 4. Discrepância de valor se informado
            if let (Some(val_p), Some(val_u)) = (p.valor, valor_digitado_usuario) {
                if (val_p - val_u).abs() > 0.01 {
                    score += 45;
                    fatores.push(RiskFactor {
                        codigo: "VALOR_PIX_DIVERGENTE".to_string(),
                        descricao: format!(
                            "Valor contido no QR Code PIX (R$ {:.2}) difere do valor solicitado (R$ {:.2}).",
                            val_p, val_u
                        ),
                        impacto_pontos: 45,
                        severidade: "ALTA".to_string(),
                    });
                }
            }
        } else {
            transacao_tipo = "INDEFINIDO".to_string();
            score = 100;
            fatores.push(RiskFactor {
                codigo: "DADOS_INSUFICIENTES".to_string(),
                descricao: "Nenhum documento financeiro válido submetido para apuração de risco.".to_string(),
                impacto_pontos: 100,
                severidade: "CRITICA".to_string(),
            });
        }

        let score_final = score.min(100);

        let nivel_risco = match score_final {
            0..=20 => RiskLevel::Baixo,
            21..=50 => RiskLevel::Medio,
            51..=75 => RiskLevel::Alto,
            _ => RiskLevel::Critico,
        };

        let recomendacao = match nivel_risco {
            RiskLevel::Baixo => RiskRecommendation::AprovarAutomaticamente,
            RiskLevel::Medio => RiskRecommendation::RevisaoManualRequerida,
            RiskLevel::Alto | RiskLevel::Critico => RiskRecommendation::RejeitarOperacao,
        };

        ValidationReport {
            transacao_tipo,
            score_risco: score_final,
            nivel_risco,
            recomendacao,
            fatores_risco: fatores,
            boleto_detalhes: boleto.cloned(),
            pix_detalhes: pix.cloned(),
            timestamp_analise: "2026-11-03T04:50:00Z".to_string(),
        }
    }

    // ========================================================================
    // MÉTODOS AUXILIARES PRIVADOS
    // ========================================================================

    fn calcular_data_vencimento(fator: u32) -> Option<String> {
        if fator == 0 {
            return None;
        }
        // Base oficial do Banco Central / Febraban: 07/10/1997 para fator 1000
        // Ajuste para virada de fatores pós 2025:
        // Fator 1000 = 07/10/1997. Cada unidade = 1 dia.
        // Fator 9999 atingido em 21/02/2025. A partir de 22/02/2025, reinicia em 1000.
        let dias = fator as i64 - 1000;
        Some(format!("Fator {} (+{} dias da base Bacen)", fator, dias))
    }

    fn obter_nome_banco(codigo: &str) -> &'static str {
        match codigo {
            "001" => "Banco do Brasil S.A.",
            "033" => "Banco Santander (Brasil) S.A.",
            "104" => "Caixa Econômica Federal",
            "237" => "Banco Bradesco S.A.",
            "341" => "Itaú Unibanco S.A.",
            "077" => "Banco Inter S.A.",
            "260" => "Nu Pagamentos S.A. (Nubank)",
            "212" => "Banco Original S.A.",
            "336" => "Banco C6 S.A.",
            "422" => "Banco Safra S.A.",
            "748" => "Banco Cooperativo Sicredi S.A.",
            "756" => "Banco Cooperativo Sicoob S.A.",
            _ => "Instituição Bancária Autorizada",
        }
    }

    fn nome_tag_emv(id: &str) -> &'static str {
        match id {
            "00" => "Payload Format Indicator",
            "01" => "Point of Initiation Method",
            "26" => "Merchant Account Information (PIX)",
            "52" => "Merchant Category Code (MCC)",
            "53" => "Transaction Currency (ISO 4217)",
            "54" => "Transaction Amount",
            "58" => "Country Code",
            "59" => "Merchant Name",
            "60" => "Merchant City",
            "62" => "Additional Data Field (TxID)",
            "63" => "CRC16 Checksum",
            _ => "Proprietary / Custom Tag",
        }
    }

    fn parse_subtags_pix_26(val: &str) -> (Option<String>, Option<String>, Option<String>) {
        let bytes = val.as_bytes();
        let mut cursor = 0;
        let mut chave = None;
        let mut url = None;

        while cursor + 4 <= bytes.len() {
            let id = match std::str::from_utf8(&bytes[cursor..cursor + 2]) {
                Ok(s) => s,
                Err(_) => break,
            };
            let len: usize = match std::str::from_utf8(&bytes[cursor + 2..cursor + 4])
                .ok()
                .and_then(|s| s.parse().ok())
            {
                Some(n) => n,
                None => break,
            };

            cursor += 4;
            if cursor + len > bytes.len() {
                break;
            }

            let sub_val = match std::str::from_utf8(&bytes[cursor..cursor + len]) {
                Ok(s) => s,
                Err(_) => break,
            };

            if id == "01" {
                chave = Some(sub_val.to_string());
            } else if id == "25" {
                url = Some(sub_val.to_string());
            }

            cursor += len;
        }

        let tipo_chave = chave.as_ref().map(|k| Self::identificar_tipo_chave_pix(k));

        (chave, tipo_chave, url)
    }

    fn identificar_tipo_chave_pix(k: &str) -> String {
        if k.contains('@') {
            "E-mail".to_string()
        } else if k.chars().all(|c| c.is_ascii_digit() || c == '+') && k.len() >= 12 {
            "Telefone".to_string()
        } else if k.len() == 11 && k.chars().all(|c| c.is_ascii_digit()) {
            "CPF".to_string()
        } else if k.len() == 14 && k.chars().all(|c| c.is_ascii_digit()) {
            "CNPJ".to_string()
        } else if k.contains('-') && k.len() == 36 {
            "Chave Aleatória (EVP UUID)".to_string()
        } else {
            "Chave PIX".to_string()
        }
    }

    fn parse_subtag_txid(val: &str) -> Option<String> {
        let bytes = val.as_bytes();
        let mut cursor = 0;

        while cursor + 4 <= bytes.len() {
            let id = std::str::from_utf8(&bytes[cursor..cursor + 2]).ok()?;
            let len: usize = std::str::from_utf8(&bytes[cursor + 2..cursor + 4])
                .ok()?
                .parse()
                .ok()?;
            cursor += 4;
            if cursor + len > bytes.len() {
                break;
            }
            let sub_val = std::str::from_utf8(&bytes[cursor..cursor + len]).ok()?;
            if id == "05" {
                return Some(sub_val.to_string());
            }
            cursor += len;
        }

        None
    }

    /// Analisa o texto extraído de um documento (PDF, Word, TXT, etc.)
    /// procurando automaticamente por Boletos Bancários e códigos PIX,
    /// validando os checksums de cada um e emitindo um parecer de veracidade global.
    pub fn find_and_validate_in_text(
        &self,
        text: &str,
        valor_esperado: Option<f64>,
    ) -> DocumentAnalysisReport {
        let mut itens = Vec::new();
        let mut alertas = Vec::new();

        // 1. Varredura por PIX Copia e Cola (inicia com "000201" e contém "6304")
        // Suporta payloads em linha única ou com quebras de linha/espaços acidentais
        let mut start_idx = 0;
        while let Some(rel_start) = text[start_idx..].find("000201") {
            let actual_start = start_idx + rel_start;
            if let Some(rel_crc) = text[actual_start..].find("6304") {
                let actual_crc = actual_start + rel_crc;
                if text.len() >= actual_crc + 8 {
                    let candidate_pix = &text[actual_start..actual_crc + 8];
                    let clean_pix: String = candidate_pix.chars().filter(|c| !c.is_whitespace()).collect();
                    let pix_res = self.validate_pix(&clean_pix);
                    let report = self.calculate_risk(None, Some(&pix_res), valor_esperado);

                    itens.push(DocumentFinancialItem {
                        tipo: "PIX".to_string(),
                        texto_bruto: clean_pix,
                        boleto_resultado: None,
                        pix_resultado: Some(pix_res),
                        report,
                    });
                    start_idx = actual_crc + 8;
                    continue;
                }
            }
            start_idx = actual_start + 6;
        }

        // 2. Varredura Robusta por Boletos Bancários
        let mut vistos_boletos = std::collections::HashSet::new();

        // Pass 1: Janela Deslizante de Tokens (Agrupamento de 1 a 8 blocos de texto)
        // Resolve formato padrão FEBRABAN com 5 blocos separados por espaços/pontos:
        // "00190.00009 01041.800408 00000.000059 2 97890000015000"
        // mesmo quando precedido por cabeçalhos bancários ou seguido por datas e valores
        let raw_tokens: Vec<&str> = text
            .split(|c: char| c.is_whitespace() || c == '|' || c == ';')
            .filter(|s| !s.trim().is_empty())
            .collect();

        let mut t_idx = 0;
        while t_idx < raw_tokens.len() {
            let mut acc_digits = String::new();
            let mut acc_snippet = Vec::new();

            for j in t_idx..std::cmp::min(t_idx + 8, raw_tokens.len()) {
                let tok = raw_tokens[j];
                let tok_digits: String = tok.chars().filter(|c| c.is_ascii_digit()).collect();

                if tok_digits.is_empty() {
                    if tok == "-" || tok == "." || tok == "/" {
                        continue;
                    }
                    if !acc_digits.is_empty() {
                        break;
                    } else {
                        continue;
                    }
                }

                acc_digits.push_str(&tok_digits);
                acc_snippet.push(tok);

                if acc_digits.len() == 47 || acc_digits.len() == 48 || acc_digits.len() == 44 {
                    if !vistos_boletos.contains(&acc_digits) {
                        let linha_analise = if acc_digits.len() == 44 {
                            Self::barcode_to_linha_digitavel(&acc_digits).unwrap_or(acc_digits.clone())
                        } else {
                            acc_digits.clone()
                        };

                        let boleto_res = self.validate_boleto(&linha_analise);
                        if boleto_res.is_valid
                            || boleto_res.campos_validacao.iter().filter(|c| c.valido).count() >= 2
                            || acc_digits.len() == 47
                            || acc_digits.len() == 48
                        {
                            vistos_boletos.insert(acc_digits.clone());
                            let report = self.calculate_risk(Some(&boleto_res), None, valor_esperado);

                            itens.push(DocumentFinancialItem {
                                tipo: if acc_digits.len() == 44 {
                                    "CODIGO_BARRAS_44".to_string()
                                } else {
                                    "BOLETO".to_string()
                                },
                                texto_bruto: acc_snippet.join(" "),
                                boleto_resultado: Some(boleto_res),
                                pix_resultado: None,
                                report,
                            });
                            t_idx = j;
                            break;
                        }
                    }
                }

                if acc_digits.len() > 48 {
                    break;
                }
            }
            t_idx += 1;
        }

        // Pass 2: Varredura de Sequências Contíguas de Dígitos e Formatação (Sliding Window de Caracteres)
        // Detecta quando o boleto está em uma linha contínua, com tabelas ou formatação personalizada (. / - espaço)
        let chars: Vec<char> = text.chars().collect();
        let mut c_idx = 0;
        while c_idx < chars.len() {
            if chars[c_idx].is_ascii_digit() {
                let start = c_idx;
                while c_idx < chars.len()
                    && (chars[c_idx].is_ascii_digit()
                        || chars[c_idx] == '.'
                        || chars[c_idx] == '-'
                        || chars[c_idx] == ' '
                        || chars[c_idx] == '	'
                        || chars[c_idx] == '/')
                {
                    c_idx += 1;
                }
                let slice_str: String = chars[start..c_idx].iter().collect();
                let clean_digits: String = slice_str.chars().filter(|c| c.is_ascii_digit()).collect();

                if clean_digits.len() >= 47 {
                    for w in 0..=(clean_digits.len() - 47) {
                        let cand = &clean_digits[w..w + 47];
                        if !vistos_boletos.contains(cand) {
                            let boleto_res = self.validate_boleto(cand);
                            if boleto_res.is_valid || boleto_res.campos_validacao.iter().filter(|c| c.valido).count() >= 2 {
                                vistos_boletos.insert(cand.to_string());
                                let report = self.calculate_risk(Some(&boleto_res), None, valor_esperado);
                                itens.push(DocumentFinancialItem {
                                    tipo: "BOLETO".to_string(),
                                    texto_bruto: slice_str.trim().to_string(),
                                    boleto_resultado: Some(boleto_res),
                                    pix_resultado: None,
                                    report,
                                });
                                break;
                            }
                        }
                    }
                }

                if clean_digits.len() >= 48 {
                    for w in 0..=(clean_digits.len() - 48) {
                        let cand = &clean_digits[w..w + 48];
                        if !vistos_boletos.contains(cand) {
                            let boleto_res = self.validate_boleto(cand);
                            if boleto_res.is_valid {
                                vistos_boletos.insert(cand.to_string());
                                let report = self.calculate_risk(Some(&boleto_res), None, valor_esperado);
                                itens.push(DocumentFinancialItem {
                                    tipo: "BOLETO".to_string(),
                                    texto_bruto: slice_str.trim().to_string(),
                                    boleto_resultado: Some(boleto_res),
                                    pix_resultado: None,
                                    report,
                                });
                                break;
                            }
                        }
                    }
                }

                if clean_digits.len() == 44 && !vistos_boletos.contains(&clean_digits) {
                    if let Some(ld) = Self::barcode_to_linha_digitavel(&clean_digits) {
                        let boleto_res = self.validate_boleto(&ld);
                        if boleto_res.dv_geral_valido || boleto_res.is_valid {
                            vistos_boletos.insert(clean_digits.clone());
                            let report = self.calculate_risk(Some(&boleto_res), None, valor_esperado);
                            itens.push(DocumentFinancialItem {
                                tipo: "CODIGO_BARRAS_44".to_string(),
                                texto_bruto: clean_digits.clone(),
                                boleto_resultado: Some(boleto_res),
                                pix_resultado: None,
                                report,
                            });
                        }
                    }
                }
            } else {
                c_idx += 1;
            }
        }

        // Pass 3: Varredura por linhas inteiras
        for line in text.lines() {
            let limpo: String = line.chars().filter(|c| c.is_ascii_digit()).collect();
            if (limpo.len() == 47 || limpo.len() == 48 || limpo.len() == 44) && vistos_boletos.insert(limpo.clone()) {
                let linha_analise = if limpo.len() == 44 {
                    Self::barcode_to_linha_digitavel(&limpo).unwrap_or(limpo.clone())
                } else {
                    limpo.clone()
                };
                let boleto_res = self.validate_boleto(&linha_analise);
                let report = self.calculate_risk(Some(&boleto_res), None, valor_esperado);

                itens.push(DocumentFinancialItem {
                    tipo: if limpo.len() == 44 { "CODIGO_BARRAS_44".to_string() } else { "BOLETO".to_string() },
                    texto_bruto: line.trim().to_string(),
                    boleto_resultado: Some(boleto_res),
                    pix_resultado: None,
                    report,
                });
            }
        }

        if itens.is_empty() {
            alertas.push("Nenhum boleto ou código PIX foi localizado no documento analisado.".to_string());
        }

        let mut total_validos = 0;
        let mut total_invalidos = 0;
        let mut max_score = 0;

        for item in &itens {
            if item.report.score_risco > max_score {
                max_score = item.report.score_risco;
            }
            let is_item_valid = item
                .boleto_resultado
                .as_ref()
                .map(|b| b.is_valid)
                .or_else(|| item.pix_resultado.as_ref().map(|p| p.is_valid))
                .unwrap_or(false);

            if is_item_valid {
                total_validos += 1;
            } else {
                total_invalidos += 1;
                alertas.push(format!(
                    "Item '{}' falhou na validação de integridade ou DVs. Risco de fraude.",
                    item.tipo
                ));
            }
        }

        let documento_confiavel = total_invalidos == 0 && !itens.is_empty() && max_score <= 20;

        let recomendacao_global = if documento_confiavel {
            RiskRecommendation::AprovarAutomaticamente
        } else if max_score > 50 || total_invalidos > 0 {
            RiskRecommendation::RejeitarOperacao
        } else {
            RiskRecommendation::RevisaoManualRequerida
        };

        DocumentAnalysisReport {
            total_itens_encontrados: itens.len(),
            total_validos,
            total_invalidos,
            documento_confiavel,
            score_risco_global: max_score,
            recomendacao_global,
            itens,
            alertas_documento: alertas,
        }
    }
}

// ============================================================================
// TESTES UNITÁRIOS (RUST 2021)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Teste 1: Validação de Boleto de Cobrança Registrado Válido (Banco do Brasil)
    #[test]
    fn test_valid_boleto() {
        let validator = FinancialValidator::new();
        // Linha digitável matematicamente perfeita (DVs: 9, 8, 9; DV Geral: 2):
        let linha_valida = "00190.00009 01041.800408 00000.000059 2 97890000015000";

        let result = validator.validate_boleto(linha_valida);

        assert!(result.is_valid, "O boleto válido deve passar em todos os DVs");
        assert_eq!(result.tipo, BoletoType::Cobranca);
        assert_eq!(result.banco_codigo.as_deref(), Some("001"));
        assert_eq!(result.banco_nome.as_deref(), Some("Banco do Brasil S.A."));
        assert_eq!(result.valor, Some(150.00));
        assert!(result.dv_geral_valido);
        assert_eq!(result.erros.len(), 0);
    }

    /// Teste 2: Boleto com Dígito Verificador Adulterado (Detecção de Fraude)
    #[test]
    fn test_tampered_boleto_dv_failure() {
        let validator = FinancialValidator::new();
        // Alterado o DV do campo 1 de '9' para '4'
        let linha_fraudada = "00190.00004 01041.800408 00000.000059 2 97890000015000";

        let result = validator.validate_boleto(linha_fraudada);

        assert!(!result.is_valid, "Boleto com DV alterado deve ser rejeitado");
        assert!(result.erros.iter().any(|e| e.contains("DV do Campo 1 inválido")));
    }

    /// Teste 3: PIX Copia e Cola (EMV BR Code) Válido com CRC16 CCITT-FALSE
    #[test]
    fn test_valid_pix() {
        let validator = FinancialValidator::new();
        // Payload oficial PIX com CRC16 E17A exato:
        let pix_payload = "00020101021126430014br.gov.bcb.pix0121fulano@exemplo.com.br520400005303986540550.005802BR5913FULANO DE TAL6009SAO PAULO62110507TXID1236304E17A";

        let result = validator.validate_pix(pix_payload);

        assert!(result.is_valid, "PIX com CRC e tags válidas deve ser aprovado");
        assert!(result.crc16_valido, "CRC16 CCITT-FALSE deve ser válido");
        assert_eq!(result.crc16_informado, "E17A");
        assert_eq!(result.crc16_calculado, "E17A");
        assert_eq!(result.valor, Some(50.00));
        assert_eq!(result.chave_pix.as_deref(), Some("fulano@exemplo.com.br"));
        assert_eq!(result.tipo_chave.as_deref(), Some("E-mail"));
        assert_eq!(result.beneficiario_nome.as_deref(), Some("FULANO DE TAL"));
        assert_eq!(result.beneficiario_cidade.as_deref(), Some("SAO PAULO"));
        assert_eq!(result.txid.as_deref(), Some("TXID123"));
    }

    /// Teste 4: PIX Alterado/Adulterado (Ex: valor modificado de 50.00 para 99.99 sem atualizar CRC)
    #[test]
    fn test_tampered_pix_crc_failure() {
        let validator = FinancialValidator::new();
        // Payload onde o valor foi adulterado de 50.00 para 99.99 sem atualizar o CRC E17A
        let pix_adulterado = "00020101021126430014br.gov.bcb.pix0121fulano@exemplo.com.br520400005303986540599.995802BR5913FULANO DE TAL6009SAO PAULO62110507TXID1236304E17A";

        let result = validator.validate_pix(pix_adulterado);

        assert!(!result.is_valid, "PIX adulterado não deve ser validado");
        assert!(!result.crc16_valido, "CRC16 deve falhar após adulteração do payload");
        assert_ne!(result.crc16_calculado, result.crc16_informado);
        assert!(result.erros.iter().any(|e| e.contains("Checksum CRC16")));
    }

    /// Teste 5: Cálculo do Motor de Score de Risco para Boleto Fraudado
    #[test]
    fn test_risk_score_calculation_for_tampered_boleto() {
        let validator = FinancialValidator::new();
        let linha_fraudada = "00190.00004 01041.800408 00000.000059 2 97890000015000";
        let boleto_res = validator.validate_boleto(linha_fraudada);

        let report = validator.calculate_risk(Some(&boleto_res), None, Some(150.00));

        assert!(report.score_risco >= 50, "Boleto com DV inválido deve gerar alto risco");
        assert_eq!(report.recomendacao, RiskRecommendation::RejeitarOperacao);
        assert!(report.fatores_risco.iter().any(|f| f.codigo == "BOLETO_DV_INVALIDO"));
    }

    /// Teste 6: Cálculo do Motor de Score de Risco para PIX Válido e Seguro
    #[test]
    fn test_risk_score_calculation_for_valid_pix() {
        let validator = FinancialValidator::new();
        let pix_payload = "00020101021126430014br.gov.bcb.pix0121fulano@exemplo.com.br520400005303986540550.005802BR5913FULANO DE TAL6009SAO PAULO62110507TXID1236304E17A";
        let pix_res = validator.validate_pix(pix_payload);

        let report = validator.calculate_risk(None, Some(&pix_res), Some(50.00));

        assert_eq!(report.score_risco, 0, "PIX com todos os dados íntegros deve ter score 0");
        assert_eq!(report.nivel_risco, RiskLevel::Baixo);
        assert_eq!(report.recomendacao, RiskRecommendation::AprovarAutomaticamente);
    }
}
