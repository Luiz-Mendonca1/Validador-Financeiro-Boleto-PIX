//! # CLI Automatizado do Validador Financeiro (Rust 2021)
//!
//! Este programa analisa automaticamente:
//! 1. Arquivo único (PDF, DOCX, TXT) especificado na rota.
//! 2. Pasta inteira com dezenas/centenas de faturas em lote.
//! 3. Lista de códigos em arquivo de texto.
//!
//! ## Como Executar no Terminal (CMD / PowerShell / Bash):
//!
//! - Analisar um arquivo único:
//!   `cargo run -- "meu_boleto.pdf"`
//!   `cargo run -- "fatura.txt" 150.00`
//!
//! - Analisar uma pasta inteira com vários documentos em lote:
//!   `cargo run -- "./documentos"`
//!
//! - Executar no modo automático interativo:
//!   `cargo run`

#![allow(unused_imports, dead_code)]

mod financial_validator;

use financial_validator::{DocumentAnalysisReport, FinancialValidator, ValidationReport};
use std::env;
use std::fs;
use std::io::{self, Read, Write};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::Instant;

use axum::{
    response::Html,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct ValidateRequest {
    codigo: String,
    valor_esperado: Option<f64>,
}

#[derive(Serialize)]
struct ValidateResponse {
    is_valid: bool,
    tipo: String,
    report: ValidationReport,
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();

    // Se argumentos forem passados pelo terminal, executa a análise CLI de arquivos/pastas
    if args.len() >= 2 {
        print_banner();
        let validator = FinancialValidator::new();
        let path = args[1].clone();
        let expected_val = args.get(2).and_then(|v| v.replace(',', ".").parse::<f64>().ok());

        let target_path = Path::new(&path);
        if !target_path.exists() {
            eprintln!("\n[ERRO] O caminho informado não existe: '{}'", path);
            eprintln!("Por favor, verifique a rota e tente novamente.");
            return;
        }

        let start_time = Instant::now();

        if target_path.is_dir() {
            // ====================================================================
            // MODO BATCH: Analisar múltiplos documentos na pasta
            // ====================================================================
            println!("\n>>> INICIANDO ANÁLISE EM LOTE NA PASTA: '{}'\n", path);
            run_batch_directory_analysis(&validator, target_path, expected_val);
        } else {
            // ====================================================================
            // MODO INDIVIDUAL: Analisar arquivo único
            // ====================================================================
            println!("\n>>> ANALISANDO DOCUMENTO ÚNICO: '{}'\n", path);
            run_single_file_analysis(&validator, target_path, expected_val);
        }

        let elapsed = start_time.elapsed();
        println!("\nProcessamento concluído em {:.2}s.", elapsed.as_secs_f64());
        return;
    }

    // Caso contrário (sem argumentos), inicia o servidor Web em localhost:3000
    print_banner();

    let app = Router::new()
        .route("/", get(page_handler))
        .route("/api/validar", post(validate_handler));

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("============================================================");
    println!("   VALIDADOR FINANCEIRO LOCAL — CYPHER LABS                 ");
    println!("   Acesse no navegador: http://localhost:3000              ");
    println!("============================================================\n");

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

// URL do seu CYPHER ADM (na Vercel ou local)
const CYPHER_ADM_URL: &str = "https://cypher-adm.vercel.app";

/// Descobre o IP real da máquina onde o validador está sendo executado
fn get_local_ip() -> String {
    if let Ok(socket) = std::net::UdpSocket::bind("0.0.0.0:0") {
        if socket.connect("8.8.8.8:80").is_ok() {
            if let Ok(addr) = socket.local_addr() {
                return addr.ip().to_string();
            }
        }
    }
    "127.0.0.1".to_string()
}

/// Envia o alerta para o CYPHER ADM apenas se o documento apresentar risco
fn notify_cypher_adm_if_risky(caminho_ou_origem: &str, report: &DocumentAnalysisReport) {
    // FILTRO: Só notifica se tiver risco real (score > 20)
    if report.score_risco_global > 20 {
        let hostname = env::var("COMPUTERNAME").unwrap_or_else(|_| "Desconhecido".into());
        let ip = get_local_ip();

        let rec_str = match report.recomendacao_global {
            financial_validator::RiskRecommendation::AprovarAutomaticamente => "Aprovar Automaticamente",
            financial_validator::RiskRecommendation::RevisaoManualRequerida => "Revisão Manual Requerida",
            financial_validator::RiskRecommendation::RejeitarOperacao => "Rejeitar Operação (Fraude Detectada)",
        };

        let fatores: Vec<String> = if report.alertas_documento.is_empty() {
            vec!["Dígitos Verificadores Inválidos / Risco de Bolware".to_string()]
        } else {
            report.alertas_documento.clone()
        };

        let payload = serde_json::json!({
            "hostname": hostname,
            "ip": ip,
            "arquivo": caminho_ou_origem,
            "confiavel": report.documento_confiavel, // <-- Campo que o servidor pediu
            "score_risco": report.score_risco_global,
            "recomendacao": rec_str,
            "fatores_risco": fatores.clone(),
            "detalhes_alerta": fatores,
        });

        println!("\n🚨 [CYPHER ALERTA] Risco detectado! Notificando o CYPHER ADM...");

        tokio::task::spawn_blocking(move || {
            let client = match reqwest::blocking::Client::builder()
                .timeout(std::time::Duration::from_secs(4))
                .build()
            {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("[!] Erro ao inicializar cliente HTTP: {:?}", e);
                    return;
                }
            };

            let endpoint = format!("{}/api/audit/financial", CYPHER_ADM_URL);

            match client.post(&endpoint).json(&payload).send() {
                Ok(resp) => {
                    if resp.status().is_success() {
                        println!("[+] Alerta entregue com sucesso ao CYPHER ADM (Status: {}).", resp.status());
                    } else {
                        eprintln!("[-] ADM respondeu com código: {}", resp.status());
                        if let Ok(text) = resp.text() {
                            eprintln!("[-] Resposta do servidor: {}", text);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("[-] Falha ao conectar ao CYPHER ADM: {:?}", e);
                }
            }
        });
    }
}

async fn validate_handler(Json(payload): Json<ValidateRequest>) -> Json<ValidateResponse> {
    let validator = FinancialValidator::new();
    let codigo = payload.codigo.trim();

    let (is_valid, tipo, report) = if codigo.starts_with("000201") {
        let pix_res = validator.validate_pix(codigo);
        let rep = validator.calculate_risk(None, Some(&pix_res), payload.valor_esperado);
        (pix_res.is_valid, "PIX".to_string(), rep)
    } else {
        let boleto_res = validator.validate_boleto(codigo);
        let rep = validator.calculate_risk(Some(&boleto_res), None, payload.valor_esperado);
        (boleto_res.is_valid, "BOLETO".to_string(), rep)
    };

    // Se for suspeito, avisa o ADM com a origem:
    let doc_report = DocumentAnalysisReport {
        score_risco_global: report.score_risco,
        alertas_documento: report.fatores_risco.iter().map(|f| f.descricao.clone()).collect(),
        total_itens_encontrados: 1,
        total_validos: if is_valid { 1 } else { 0 },
        total_invalidos: if is_valid { 0 } else { 1 },
        documento_confiavel: is_valid && report.score_risco <= 20,
        recomendacao_global: report.recomendacao.clone(),
        itens: Vec::new(),
    };
    notify_cypher_adm_if_risky("Validação Manual Localhost", &doc_report);

    Json(ValidateResponse {
        is_valid,
        tipo,
        report,
    })
}

async fn page_handler() -> Html<&'static str> {
    Html(r#"<!DOCTYPE html>
<html lang="pt-BR">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Validador de Boletos // CYPHER</title>
    <script src="https://cdn.tailwindcss.com"></script>
    <link href="https://fonts.googleapis.com/css2?family=Plus+Jakarta+Sans:wght@400;600;700;800&family=JetBrains+Mono:wght@400;600&display=swap" rel="stylesheet">
    <style>
        body { font-family: 'Plus Jakarta Sans', sans-serif; }
        .mono { font-family: 'JetBrains Mono', monospace; }
    </style>
</head>
<body class="bg-[#0b101b] text-slate-100 min-h-screen flex flex-col justify-center items-center p-4 relative overflow-x-hidden">

    <div class="w-full max-w-xl bg-[#121829] border border-slate-800 rounded-2xl p-6 sm:p-8 shadow-2xl z-10">
        <div class="flex items-center gap-3 mb-6 pb-4 border-b border-slate-800">
            <div class="w-9 h-9 rounded-xl bg-[#1D59F2] flex items-center justify-center font-bold text-white shadow-lg shadow-blue-500/20">
                ✓
            </div>
            <div>
                <h1 class="text-lg font-bold text-white tracking-tight">Validador de Boletos & PIX</h1>
                <p class="text-xs text-slate-400">Verificação matemática de DVs, Módulo 10/11 e integridade antifraude</p>
            </div>
        </div>

        <form onsubmit="handleValidate(event)" class="space-y-4">
            <div>
                <label class="block text-xs font-semibold uppercase tracking-wider text-slate-400 mb-1.5">
                    Linha Digitável, Código de Barras ou PIX
                </label>
                <textarea id="codigo" rows="3" required placeholder="Cole aqui o código (47D, 48D, 44D ou 000201...)"
                    class="w-full bg-[#070b14] border border-slate-700/80 rounded-xl px-4 py-3 text-sm text-white placeholder-slate-500 focus:outline-none focus:border-[#1D59F2] focus:ring-1 focus:ring-[#1D59F2] mono transition-all"></textarea>
            </div>

            <div>
                <label class="block text-xs font-semibold uppercase tracking-wider text-slate-400 mb-1.5">
                    Valor Esperado em R$ (Opcional - para conferir divergência)
                </label>
                <input id="valor" type="number" step="0.01" placeholder="Ex: 150.00"
                    class="w-full bg-[#070b14] border border-slate-700/80 rounded-xl px-4 py-2.5 text-sm text-white placeholder-slate-500 focus:outline-none focus:border-[#1D59F2] mono transition-all">
            </div>

            <button type="submit"
                class="w-full bg-[#1D59F2] hover:bg-[#1546cb] text-white font-bold text-sm py-3 px-4 rounded-xl transition-all shadow-lg shadow-blue-500/25 flex items-center justify-center gap-2">
                <span>Validar Autenticidade</span>
            </button>
        </form>

        <div id="resultado" class="hidden mt-6 pt-5 border-t border-slate-800"></div>
    </div>

    <!-- Marca-d'água oficial CYPHER no canto inferior direito -->
    <div class="fixed bottom-5 right-6 z-20 flex items-center gap-2.5 hover:opacity-90 transition-opacity duration-300 pointer-events-none select-none">
        <svg viewBox="0 0 215 203" class="w-7 h-7 fill-white">
            <circle cx="109.04" cy="37.80" r="10.69"/>
            <circle cx="134.69" cy="30.12" r="2.26"/>
            <circle cx="149.44" cy="34.40" r="2.82"/>
            <circle cx="84.46" cy="42.17" r="7.92"/>
            <circle cx="133.61" cy="45.60" r="7.74"/>
            <circle cx="163.79" cy="44.72" r="5.67"/>
            <circle cx="61.37" cy="52.54" r="8.21"/>
            <circle cx="150.65" cy="60.04" r="5.56"/>
            <circle cx="103.71" cy="57.50" r="2.11"/>
            <circle cx="177.30" cy="63.90" r="8.48"/>
            <circle cx="90.63" cy="60.63" r="3.09"/>
            <circle cx="44.00" cy="67.43" r="4.41"/>
            <circle cx="75.94" cy="68.85" r="4.58"/>
            <circle cx="159.40" cy="74.96" r="2.82"/>
            <circle cx="59.25" cy="84.62" r="7.78"/>
            <circle cx="181.56" cy="90.00" r="11.30"/>
            <circle cx="36.77" cy="84.43" r="3.34"/>
            <circle cx="162.80" cy="87.50" r="1.78"/>
            <circle cx="45.82" cy="110.67" r="12.68"/>
            <circle cx="174.22" cy="116.86" r="8.00"/>
            <circle cx="69.00" cy="112.50" r="2.88"/>
            <circle cx="189.50" cy="121.20" r="1.78"/>
            <circle cx="71.70" cy="128.09" r="4.85"/>
            <circle cx="47.32" cy="137.84" r="8.76"/>
            <circle cx="159.30" cy="134.35" r="5.08"/>
            <circle cx="183.72" cy="134.83" r="3.83"/>
            <circle cx="81.65" cy="143.48" r="5.29"/>
            <circle cx="143.50" cy="143.50" r="3.19"/>
            <circle cx="128.88" cy="146.59" r="2.33"/>
            <circle cx="172.10" cy="149.28" r="5.78"/>
            <circle cx="62.30" cy="156.72" r="6.89"/>
            <circle cx="97.94" cy="158.25" r="8.18"/>
            <circle cx="125.15" cy="167.31" r="13.04"/>
            <circle cx="152.26" cy="162.21" r="8.46"/>
            <circle cx="77.16" cy="169.19" r="3.70"/>
            <circle cx="91.80" cy="175.20" r="2.19"/>
        </svg>
        <span class="text-xl font-extrabold tracking-tight text-white font-sans lowercase">cypher</span>
    </div>

    <script>
        async function handleValidate(e) {
            e.preventDefault();
            const codigo = document.getElementById('codigo').value.trim();
            const valorInput = document.getElementById('valor').value.trim();
            const valor_esperado = valorInput ? parseFloat(valorInput) : null;
            const resDiv = document.getElementById('resultado');

            resDiv.innerHTML = '<p class="text-xs text-slate-400 mono">Processando checksums...</p>';
            resDiv.classList.remove('hidden');

            try {
                const response = await fetch('/api/validar', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ codigo, valor_esperado })
                });

                const data = await response.json();
                renderResult(data);
            } catch (err) {
                resDiv.innerHTML = '<p class="text-xs text-rose-400">Erro ao comunicar com o servidor local.</p>';
            }
        }

        function renderResult(data) {
            const resDiv = document.getElementById('resultado');
            const rep = data.report;
            const isValid = data.is_valid;

            let html = `
                <div class="p-4 rounded-xl border ${isValid ? 'bg-emerald-500/10 border-emerald-500/30' : 'bg-rose-500/10 border-rose-500/30'} mb-4">
                    <div class="flex items-center justify-between mb-2">
                        <span class="text-xs font-bold uppercase tracking-wider ${isValid ? 'text-emerald-400' : 'text-rose-400'}">
                            ${isValid ? '✓ DOCUMENTO VÁLIDO' : '⚠ FRAUDE / CHECKSUM INVÁLIDO'}
                        </span>
                        <span class="text-xs mono font-bold text-slate-300">Score Risco: ${rep.score_risco}/100</span>
                    </div>
                    <p class="text-xs text-slate-300 font-medium">Recomendação: <strong class="uppercase text-white">${rep.recomendacao.replace(/_/g, ' ')}</strong></p>
                </div>
            `;

            if (rep.boleto_detalhes) {
                const b = rep.boleto_detalhes;
                html += `
                    <div class="space-y-2 text-xs mono bg-[#070b14] p-4 rounded-xl border border-slate-800 text-slate-300">
                        <div>Banco Emissor: <strong class="text-white">${b.banco_nome || 'N/D'} (${b.banco_codigo || 'N/D'})</strong></div>
                        <div>Valor no Boleto: <strong class="text-white">${b.valor !== null ? 'R$ ' + b.valor.toFixed(2) : 'Aberto'}</strong></div>
                        <div>Vencimento: <strong class="text-white">${b.data_vencimento_estimada || 'Não especificado'}</strong></div>
                        <div>DV Geral: <strong class="${b.dv_geral_valido ? 'text-emerald-400' : 'text-rose-400'}">${b.dv_geral_valido ? 'VÁLIDO' : 'INVÁLIDO'}</strong></div>
                    </div>
                `;
            }

            if (rep.pix_detalhes) {
                const p = rep.pix_detalhes;
                html += `
                    <div class="space-y-2 text-xs mono bg-[#070b14] p-4 rounded-xl border border-slate-800 text-slate-300">
                        <div>Beneficiário: <strong class="text-white">${p.beneficiario_nome || 'N/D'}</strong></div>
                        <div>Cidade: <strong class="text-white">${p.beneficiario_cidade || 'N/D'}</strong></div>
                        <div>Chave PIX: <strong class="text-white">${p.chave_pix || 'N/D'}</strong> (${p.tipo_chave || 'N/D'})</div>
                        <div>Valor PIX: <strong class="text-white">${p.valor !== null ? 'R$ ' + p.valor.toFixed(2) : 'Aberto'}</strong></div>
                        <div>CRC16: <strong class="${p.crc16_valido ? 'text-emerald-400' : 'text-rose-400'}">${p.crc16_informado} (${p.crc16_valido ? 'VÁLIDO' : 'ADULTERADO'})</strong></div>
                    </div>
                `;
            }

            if (rep.fatores_risco && rep.fatores_risco.length > 0) {
                html += `
                    <div class="mt-3 space-y-1">
                        <span class="text-[11px] font-bold text-rose-400 uppercase tracking-wider block">Fatores de Risco Encontrados:</span>
                        ${rep.fatores_risco.map(f => `
                            <div class="text-xs text-rose-300 bg-rose-950/30 p-2 rounded border border-rose-900/40">
                                <strong>[${f.codigo}]</strong> ${f.descricao}
                            </div>
                        `).join('')}
                    </div>
                `;
            }

            resDiv.innerHTML = html;
        }
    </script>
</body>
</html>"#)
}

/// Exibe o cabeçalho do sistema
fn print_banner() {
    println!("====================================================================");
    println!("   VALIDADOR FINANCEIRO BATCH & CLI — RUST 2021 ANTIFRAUDE         ");
    println!("   FEBRABAN (Cobrança & Concessionárias) | BACEN PIX (EMV QR Code) ");
    println!("====================================================================");
}

/// Solicita ao usuário a rota caso não tenha sido passada por argumento
fn prompt_user_for_route() -> (String, Option<f64>) {
    // Cria pasta padrão de exemplo se não existir
    let _ = fs::create_dir_all("documentos");
    create_sample_files_if_empty("documentos");

    println!("Digite o caminho do arquivo ou pasta que deseja analisar:");
    println!("(Exemplo: 'documentos' ou 'fatura.pdf' ou 'boletos.txt')");
    print!("Rota [padrão: documentos]: ");
    let _ = io::stdout().flush();

    let mut input = String::new();
    let _ = io::stdin().read_line(&mut input);
    let trimmed = input.trim();

    let rota = if trimmed.is_empty() {
        "documentos".to_string()
    } else {
        trimmed.to_string()
    };

    (rota, None)
}

/// Cria arquivos de exemplo para testes imediatos
fn create_sample_files_if_empty(dir: &str) {
    let f1 = format!("{}/01_boleto_banco_brasil_valido.txt", dir);
    let f2 = format!("{}/02_boleto_fraudado_dv_adulterado.txt", dir);
    let f3 = format!("{}/03_pix_copia_e_cola_valido.txt", dir);

    if !Path::new(&f1).exists() {
        let _ = fs::write(
            &f1,
            "FATURA DE SERVICOS - BANCO DO BRASIL\n\
             Vencimento: 15/10/2026 | Valor: R$ 150,00\n\
             Linha Digitavel:\n\
             00190.00009 01041.800408 00000.000059 2 97890000015000\n",
        );
    }
    if !Path::new(&f2).exists() {
        let _ = fs::write(
            &f2,
            "FATURA SUSPEITA - TENTATIVA DE FRAUDE BOLWARE\n\
             Linha Digitavel com DV adulterado de 9 para 4:\n\
             00190.00004 01041.800408 00000.000059 2 97890000015000\n",
        );
    }
    if !Path::new(&f3).exists() {
        let _ = fs::write(
            &f3,
            "PAGAMENTO VIA PIX QR CODE ESTÁTICO\n\
             Beneficiário: FULANO DE TAL | Cidade: SAO PAULO\n\
             Código Copia e Cola:\n\
             00020101021126430014br.gov.bcb.pix0121fulano@exemplo.com.br520400005303986540550.005802BR5913FULANO DE TAL6009SAO PAULO62110507TXID1236304E17A\n",
        );
    }
}

/// Executa análise de um único arquivo
fn run_single_file_analysis(validator: &FinancialValidator, file_path: &Path, valor_esperado: Option<f64>) {
    let text = extract_text_from_file(file_path);
    if text.trim().is_empty() {
        println!("[-] O arquivo '{}' está vazio ou o texto não pôde ser lido.", file_path.display());
        return;
    }

    let report = validator.find_and_validate_in_text(&text, valor_esperado);
    print_report_summary(file_path, &report);
    notify_cypher_adm_if_risky(&file_path.to_string_lossy(), &report);
}

/// Executa análise recursiva em lote de todos os arquivos de um diretório
fn run_batch_directory_analysis(validator: &FinancialValidator, dir_path: &Path, valor_esperado: Option<f64>) {
    let mut files_to_scan = Vec::new();
    collect_files_recursively(dir_path, &mut files_to_scan);

    if files_to_scan.is_empty() {
        println!("[-] Nenhum arquivo encontrado no diretório '{}'.", dir_path.display());
        return;
    }

    println!("Total de arquivos identificados para auditoria: {}\n", files_to_scan.len());

    let mut total_docs_analisados = 0;
    let mut total_docs_aprovados = 0;
    let mut total_docs_suspeitos = 0;
    let mut total_itens_financeiros = 0;
    let mut relatorio_consolidado: Vec<serde_json::Value> = Vec::new();

    for (idx, file) in files_to_scan.iter().enumerate() {
        println!("--------------------------------------------------------------------");
        println!("[{}/{}] Arquivo: {}", idx + 1, files_to_scan.len(), file.display());

        let text = extract_text_from_file(file);
        if text.trim().is_empty() {
            println!("  -> [AVISO] Arquivo vazio ou sem texto detectável.");
            continue;
        }

        let report = validator.find_and_validate_in_text(&text, valor_esperado);
        total_docs_analisados += 1;
        total_itens_financeiros += report.total_itens_encontrados;

        if report.documento_confiavel {
            total_docs_aprovados += 1;
            println!("  -> STATUS: [APROVADO / SEGURO] Score: {}/100", report.score_risco_global);
        } else {
            total_docs_suspeitos += 1;
            println!("  -> STATUS: [REJEITADO / ALTO RISCO] Score: {}/100", report.score_risco_global);
            notify_cypher_adm_if_risky(&file.to_string_lossy(), &report);
        }

        println!("  -> Itens encontrados: {} (Válidos: {}, Inválidos: {})", 
                 report.total_itens_encontrados, report.total_validos, report.total_invalidos);

        for (i, item) in report.itens.iter().enumerate() {
            print!("     #{} [{}]: ", i + 1, item.tipo);
            if let Some(b) = &item.boleto_resultado {
                println!("Banco: {:?} | R$ {:?} | Válido: {}", 
                         b.banco_nome.as_deref().unwrap_or("N/D"), b.valor, b.is_valid);
            } else if let Some(p) = &item.pix_resultado {
                println!("Beneficiário: {:?} | R$ {:?} | CRC16: {}", 
                         p.beneficiario_nome.as_deref().unwrap_or("N/D"), p.valor, p.crc16_calculado);
            }
        }

        if !report.alertas_documento.is_empty() {
            for alerta in &report.alertas_documento {
                println!("     ! Alerta: {}", alerta);
            }
        }

        // Adiciona ao relatório JSON consolidado
        relatorio_consolidado.push(serde_json::json!({
            "arquivo": file.to_string_lossy(),
            "confiavel": report.documento_confiavel,
            "score_risco": report.score_risco_global,
            "recomendacao": report.recomendacao_global,
            "total_itens": report.total_itens_encontrados,
            "itens_validos": report.total_validos,
            "itens_invalidos": report.total_invalidos,
            "alertas": report.alertas_documento,
        }));
    }

    // Salva o relatório consolidado em JSON
    let output_json_name = "relatorio_auditoria_batch.json";
    if let Ok(json_str) = serde_json::to_string_pretty(&relatorio_consolidado) {
        let _ = fs::write(output_json_name, json_str);
        println!("\n[+] Relatório consolidado em JSON salvo com sucesso em: '{}'", output_json_name);
    }

    // Resumo final do lote
    println!("====================================================================");
    println!("                  RESUMO DA AUDITORIA EM LOTE                       ");
    println!("====================================================================");
    println!("Total de documentos analisados: {}", total_docs_analisados);
    println!("Documentos Seguros / Aprovados: {}", total_docs_aprovados);
    println!("Documentos Suspeitos / Reprovados:  {}", total_docs_suspeitos);
    println!("Total de boletos e PIX auditados:   {}", total_itens_financeiros);
    println!("====================================================================");
}

/// Imprime os detalhes de um único relatório
fn print_report_summary(path: &Path, report: &DocumentAnalysisReport) {
    println!("--------------------------------------------------------------------");
    println!("Documento: {}", path.display());
    println!("Status Global: {}", if report.documento_confiavel { "[+] CONFIÁVEL / ÍNTEGRO" } else { "[!] SUSPEITO / RISCO DETECTADO" });
    println!("Score de Risco Antifraude: {}/100", report.score_risco_global);
    println!("Recomendação do Motor: {:?}", report.recomendacao_global);
    println!("Total de Itens Financeiros Encontrados: {}", report.total_itens_encontrados);
    println!("  -> Válidos: {} | Inválidos/Adulterados: {}", report.total_validos, report.total_invalidos);

    for (i, item) in report.itens.iter().enumerate() {
        println!("\n  [Item #{}] Tipo: {}", i + 1, item.tipo);
        println!("  Texto Bruto: {}", item.texto_bruto);
        if let Some(boleto) = &item.boleto_resultado {
            println!("    Banco Emissor: {}", boleto.banco_nome.as_deref().unwrap_or("Não identificado"));
            println!("    Linha Limpa (47D/48D): {}", boleto.linha_digitavel_limpa);
            if let Some(cb) = &boleto.codigo_barras {
                println!("    Código de Barras (44D): {}", cb);
            }
            if let Some(v) = boleto.valor {
                println!("    Valor no Boleto: R$ {:.2}", v);
            }
            if let Some(venc) = &boleto.data_vencimento_estimada {
                println!("    Vencimento Estimado: {}", venc);
            }
            println!("    DVs dos Blocos: {}", if boleto.campos_validacao.iter().all(|c| c.valido) { "VÁLIDOS (Módulo 10/11)" } else { "INVÁLIDOS / ADULTERADOS" });
            println!("    DV Geral da Barra: {}", if boleto.dv_geral_valido { "VÁLIDO (Módulo 11)" } else { "INVÁLIDO" });
            println!("    Status Final: {}", if boleto.is_valid { "[APROVADO] Boleto Autêntico" } else { "[REJEITADO] Boleto Fraudado / Bolware" });
        }
        if let Some(pix) = &item.pix_resultado {
            println!("    Beneficiário: {}", pix.beneficiario_nome.as_deref().unwrap_or("N/D"));
            println!("    Cidade: {}", pix.beneficiario_cidade.as_deref().unwrap_or("N/D"));
            println!("    Chave PIX: {} ({})", pix.chave_pix.as_deref().unwrap_or("N/D"), pix.tipo_chave.as_deref().unwrap_or("N/D"));
            if let Some(v) = pix.valor {
                println!("    Valor PIX: R$ {:.2}", v);
            }
            println!("    Checksum CRC16: Calculado: {} | Informado: {} [{}]",
                     pix.crc16_calculado, pix.crc16_informado, if pix.crc16_valido { "VÁLIDO" } else { "ADULTERADO" });
            println!("    Status Final: {}", if pix.is_valid { "[APROVADO] PIX Autêntico" } else { "[REJEITADO] PIX Corrompido / Man-in-the-Middle" });
        }
    }

    if !report.alertas_documento.is_empty() {
        println!("\nAlertas de Auditoria:");
        for alerta in &report.alertas_documento {
            println!("  [!] {}", alerta);
        }
    }
    println!("--------------------------------------------------------------------");
}

/// Extrai texto completo de qualquer documento suportado (PDF, Word DOCX/DOC, TXT, etc.)
/// Equivalente aos extratores do modelo web (PDF.js + Mammoth).
fn extract_text_from_file(path: &Path) -> String {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    // 1. Tenta leitura UTF-8 para arquivos de texto puro
    if ext == "txt" || ext == "csv" || ext == "json" || ext == "log" || ext == "emv" {
        if let Ok(text) = fs::read_to_string(path) {
            if !text.trim().is_empty() {
                return text;
            }
        }
    }

    // 2. Lê os bytes do arquivo
    let bytes = match fs::read(path) {
        Ok(b) => b,
        Err(_) => return String::new(),
    };

    // 3. Documento PDF (extensão .pdf ou cabeçalho %PDF)
    if ext == "pdf" || bytes.starts_with(b"%PDF") || bytes.windows(4).any(|w| w == b"%PDF") {
        let pdf_text = extract_text_from_pdf(path, &bytes);
        if !pdf_text.trim().is_empty() {
            return pdf_text;
        }
    }

    // 4. Documento Word DOCX (.docx ou arquivo ZIP com cabeçalho PK\x03\x04)
    if ext == "docx" || bytes.starts_with(b"PK\x03\x04") {
        let docx_text = extract_text_from_docx(&bytes);
        if !docx_text.trim().is_empty() {
            return docx_text;
        }
    }

    // 5. Documento Word DOC Legado (.doc / cabeçalho OLE CFB 0xD0, 0xCF, 0x11, 0xE0)
    if ext == "doc" || (bytes.len() >= 4 && bytes[0] == 0xD0 && bytes[1] == 0xCF && bytes[2] == 0x11 && bytes[3] == 0xE0) {
        let doc_text = extract_text_from_legacy_doc(&bytes);
        if !doc_text.trim().is_empty() {
            return doc_text;
        }
    }

    // 6. Tenta decodificação UTF-8 ou Latin-1
    if let Ok(text) = std::str::from_utf8(&bytes) {
        if !text.trim().is_empty() {
            return text.to_string();
        }
    }

    // 7. Fallback geral para varredura de sequências ASCII imprimíveis
    extract_ascii_strings_fallback(&bytes)
}

/// Extrai texto de PDFs utilizando motor multicamada profissional:
/// Camada 1: Crate especializada 'pdf-extract' (decodifica fontes embutidas, ToUnicode CMaps, TrueType e Type1)
/// Camada 2: Descompressão nativa de streams FlateDecode via miniz_oxide e operadores PDF (Tj, TJ, hex)
/// Camada 3: Varredura profunda de strings ASCII/UTF-8
fn extract_text_from_pdf(path: &Path, bytes: &[u8]) -> String {
    // Camada 1: Tenta extrair com pdf-extract em memória
    if let Ok(text) = pdf_extract::extract_text_from_mem(bytes) {
        if text.trim().len() > 10 {
            return text;
        }
    }

    // Tenta extrair com pdf-extract direto do caminho do arquivo
    if let Ok(text) = pdf_extract::extract_text(path) {
        if text.trim().len() > 10 {
            return text;
        }
    }

    // Camada 2: Descompressão nativa de streams PDF (/FlateDecode)
    let stream_text = extract_text_from_pdf_streams(bytes);
    if stream_text.trim().len() > 10 {
        return stream_text;
    }

    // Camada 3: Fallback de sequências ASCII
    extract_ascii_strings_fallback(bytes)
}

/// Varre o PDF em busca de streams de mapas de caracteres (/ToUnicode CMaps)
/// Essencial para decodificar PDFs bancários reais com fontes CID/Type0 embutidas
fn parse_pdf_cmaps(bytes: &[u8]) -> std::collections::HashMap<u16, char> {
    let mut cmap = std::collections::HashMap::new();
    let stream_keyword = b"stream";
    let endstream_keyword = b"endstream";
    let mut cursor = 0;

    while cursor < bytes.len() {
        if let Some(pos) = bytes[cursor..].windows(stream_keyword.len()).position(|w| w == stream_keyword) {
            let stream_start = cursor + pos + stream_keyword.len();
            let mut data_start = stream_start;
            while data_start < bytes.len() && (bytes[data_start] == b'\r' || bytes[data_start] == b'\n') {
                data_start += 1;
            }

            if let Some(end_pos) = bytes[data_start..].windows(endstream_keyword.len()).position(|w| w == endstream_keyword) {
                let mut data_end = data_start + end_pos;
                while data_end > data_start && (bytes[data_end - 1] == b'\r' || bytes[data_end - 1] == b'\n') {
                    data_end -= 1;
                }

                if data_end > data_start {
                    let stream_data = &bytes[data_start..data_end];
                    let decomp = miniz_oxide::inflate::decompress_to_vec_zlib(stream_data)
                        .or_else(|_| miniz_oxide::inflate::decompress_to_vec(stream_data));

                    if let Ok(decomp_bytes) = decomp {
                        if let Ok(text) = std::str::from_utf8(&decomp_bytes) {
                            if text.contains("beginbfchar") || text.contains("beginbfrange") {
                                parse_single_cmap_stream(text, &mut cmap);
                            }
                        }
                    }
                }
                cursor = data_end + endstream_keyword.len();
            } else {
                break;
            }
        } else {
            break;
        }
    }

    cmap
}

/// Extrai mapeamentos de um stream CMap individual (beginbfchar e beginbfrange)
fn parse_single_cmap_stream(text: &str, cmap: &mut std::collections::HashMap<u16, char>) {
    // 1. beginbfchar: <CID_HEX> <UNICODE_HEX>
    let mut cursor = 0;
    while let Some(idx) = text[cursor..].find("beginbfchar") {
        let sec_start = cursor + idx + 11;
        if let Some(end_rel) = text[sec_start..].find("endbfchar") {
            let sec = &text[sec_start..sec_start + end_rel];
            let tokens: Vec<&str> = sec.split_whitespace().collect();
            let mut i = 0;
            while i + 1 < tokens.len() {
                let t1 = tokens[i].trim_matches(|c| c == '<' || c == '>');
                let t2 = tokens[i + 1].trim_matches(|c| c == '<' || c == '>');
                if let (Ok(src), Ok(dst)) = (u16::from_str_radix(t1, 16), u32::from_str_radix(t2, 16)) {
                    if let Some(ch) = char::from_u32(dst) {
                        cmap.insert(src, ch);
                    }
                }
                i += 2;
            }
            cursor = sec_start + end_rel + 9;
        } else {
            break;
        }
    }

    // 2. beginbfrange: <START_CID_HEX> <END_CID_HEX> <START_UNICODE_HEX>
    cursor = 0;
    while let Some(idx) = text[cursor..].find("beginbfrange") {
        let sec_start = cursor + idx + 12;
        if let Some(end_rel) = text[sec_start..].find("endbfrange") {
            let sec = &text[sec_start..sec_start + end_rel];
            let tokens: Vec<&str> = sec.split_whitespace().collect();
            let mut i = 0;
            while i + 2 < tokens.len() {
                let t1 = tokens[i].trim_matches(|c| c == '<' || c == '>');
                let t2 = tokens[i + 1].trim_matches(|c| c == '<' || c == '>');
                let t3 = tokens[i + 2].trim_matches(|c| c == '<' || c == '>');
                if let (Ok(s1), Ok(s2), Ok(dst_start)) = (
                    u16::from_str_radix(t1, 16),
                    u16::from_str_radix(t2, 16),
                    u32::from_str_radix(t3, 16),
                ) {
                    for (offset, c) in (s1..=s2).enumerate() {
                        if let Some(ch) = char::from_u32(dst_start + offset as u32) {
                            cmap.insert(c, ch);
                        }
                    }
                }
                i += 3;
            }
            cursor = sec_start + end_rel + 10;
        } else {
            break;
        }
    }
}

/// Extrai texto de todos os streams descompactados de um PDF aplicando ToUnicode CMaps
fn extract_text_from_pdf_streams(bytes: &[u8]) -> String {
    let cmap = parse_pdf_cmaps(bytes);
    let mut extracted = String::new();
    let stream_keyword = b"stream";
    let endstream_keyword = b"endstream";
    let mut cursor = 0;

    while cursor < bytes.len() {
        if let Some(pos) = bytes[cursor..].windows(stream_keyword.len()).position(|w| w == stream_keyword) {
            let stream_start = cursor + pos + stream_keyword.len();
            let mut data_start = stream_start;
            while data_start < bytes.len() && (bytes[data_start] == b'\r' || bytes[data_start] == b'\n') {
                data_start += 1;
            }

            if let Some(end_pos) = bytes[data_start..].windows(endstream_keyword.len()).position(|w| w == endstream_keyword) {
                let mut data_end = data_start + end_pos;
                while data_end > data_start && (bytes[data_end - 1] == b'\r' || bytes[data_end - 1] == b'\n') {
                    data_end -= 1;
                }

                if data_end > data_start {
                    let stream_data = &bytes[data_start..data_end];

                    let decompressed = miniz_oxide::inflate::decompress_to_vec_zlib(stream_data)
                        .or_else(|_| miniz_oxide::inflate::decompress_to_vec(stream_data));

                    if let Ok(decomp_bytes) = decompressed {
                        parse_pdf_stream_text(&decomp_bytes, &cmap, &mut extracted);
                        extracted.push('\n');
                        let fallback = extract_ascii_strings_fallback(&decomp_bytes);
                        if !fallback.trim().is_empty() {
                            extracted.push(' ');
                            extracted.push_str(&fallback);
                            extracted.push('\n');
                        }
                    } else {
                        parse_pdf_stream_text(stream_data, &cmap, &mut extracted);
                        extracted.push('\n');
                    }
                }

                cursor = data_end + endstream_keyword.len();
            } else {
                break;
            }
        } else {
            break;
        }
    }

    extracted
}

/// Interpreta operadores de texto em streams PDF descomprimidos (Tj, TJ, hex strings e escapes com CMap)
fn parse_pdf_stream_text(stream: &[u8], cmap: &std::collections::HashMap<u16, char>, out: &mut String) {
    let mut i = 0;
    while i < stream.len() {
        if stream[i] == b'(' {
            i += 1;
            let mut paren_depth = 1;
            let mut s = String::new();
            while i < stream.len() && paren_depth > 0 {
                if stream[i] == b'\\' && i + 1 < stream.len() {
                    i += 1;
                    match stream[i] {
                        b'n' => s.push('\n'),
                        b'r' => s.push('\r'),
                        b't' => s.push('\t'),
                        b'\\' => s.push('\\'),
                        b'(' => s.push('('),
                        b')' => s.push(')'),
                        b'0'..=b'7' => {
                            let mut octal = (stream[i] - b'0') as u32;
                            if i + 1 < stream.len() && stream[i + 1] >= b'0' && stream[i + 1] <= b'7' {
                                i += 1;
                                octal = octal * 8 + (stream[i] - b'0') as u32;
                                if i + 1 < stream.len() && stream[i + 1] >= b'0' && stream[i + 1] <= b'7' {
                                    i += 1;
                                    octal = octal * 8 + (stream[i] - b'0') as u32;
                                }
                            }
                            if let Some(ch) = char::from_u32(octal) {
                                s.push(ch);
                            }
                        }
                        other => s.push(other as char),
                    }
                } else if stream[i] == b'(' {
                    paren_depth += 1;
                    s.push('(');
                } else if stream[i] == b')' {
                    paren_depth -= 1;
                    if paren_depth > 0 {
                        s.push(')');
                    }
                } else {
                    s.push(stream[i] as char);
                }
                i += 1;
            }
            if !s.is_empty() {
                out.push_str(&s);
                out.push(' ');
            }
        } else if stream[i] == b'<' && i + 1 < stream.len() && stream[i + 1] != b'<' {
            // Hex string <...> (usada para textos em fontes CID / Type0)
            i += 1;
            let mut hex = String::new();
            while i < stream.len() && stream[i] != b'>' {
                if !stream[i].is_ascii_whitespace() {
                    hex.push(stream[i] as char);
                }
                i += 1;
            }
            if i < stream.len() && stream[i] == b'>' {
                i += 1;
            }

            if hex.len() % 4 == 0 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
                // CID de 2 bytes (ou UTF-16BE) decodificado via CMap
                let mut decoded = String::new();
                for chunk in hex.as_bytes().chunks(4) {
                    if let Ok(val) = u16::from_str_radix(std::str::from_utf8(chunk).unwrap_or(""), 16) {
                        if let Some(&ch) = cmap.get(&val) {
                            decoded.push(ch);
                        } else if val >= 32 && val <= 126 {
                            decoded.push((val as u8) as char);
                        }
                    }
                }
                if !decoded.is_empty() {
                    out.push_str(&decoded);
                }
            } else if hex.len() % 2 == 0 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
                let mut decoded = String::new();
                for chunk in hex.as_bytes().chunks(2) {
                    if let Ok(byte_val) = u8::from_str_radix(std::str::from_utf8(chunk).unwrap_or(""), 16) {
                        let val16 = byte_val as u16;
                        if let Some(&ch) = cmap.get(&val16) {
                            decoded.push(ch);
                        } else if byte_val >= 32 && byte_val <= 126 {
                            decoded.push(byte_val as char);
                        }
                    }
                }
                if !decoded.is_empty() {
                    out.push_str(&decoded);
                }
            }
        } else {
            i += 1;
        }
    }
}

/// Extrai texto de arquivos Word (.docx) compatível com Mammoth (lê word/document.xml)
fn extract_text_from_docx(bytes: &[u8]) -> String {
    // Camada 1: Crate zip oficial
    let cursor = std::io::Cursor::new(bytes);
    if let Ok(mut archive) = zip::ZipArchive::new(cursor) {
        let mut full_text = String::new();
        let targets = ["word/document.xml", "word/document2.xml", "word/footnotes.xml", "word/endnotes.xml"];
        for target in &targets {
            if let Ok(mut file) = archive.by_name(target) {
                let mut xml_content = String::new();
                if file.read_to_string(&mut xml_content).is_ok() {
                    let parsed = parse_word_xml_text(&xml_content);
                    if !parsed.is_empty() {
                        full_text.push_str(&parsed);
                        full_text.push('\n');
                    }
                }
            }
        }
        if full_text.trim().len() > 5 {
            return full_text;
        }
    }

    // Camada 2: Fallback manual por busca no Central Directory do ZIP
    let manual = extract_text_from_docx_zip_manual(bytes);
    if manual.trim().len() > 5 {
        return manual;
    }

    extract_ascii_strings_fallback(bytes)
}

/// Fallback manual para arquivos DOCX via varredura do Central Directory do ZIP
/// (Lê o diretório central onde o tamanho comprimido e offset são garantidos mesmo com bit 3 ativo)
fn extract_text_from_docx_zip_manual(bytes: &[u8]) -> String {
    let cdir_header = b"PK\x01\x02";
    let mut cursor = 0;
    while cursor + 46 <= bytes.len() {
        if &bytes[cursor..cursor + 4] == cdir_header {
            let comp_method = u16::from_le_bytes([bytes[cursor + 10], bytes[cursor + 11]]);
            let comp_size = u32::from_le_bytes([
                bytes[cursor + 20],
                bytes[cursor + 21],
                bytes[cursor + 22],
                bytes[cursor + 23],
            ]) as usize;
            let name_len = u16::from_le_bytes([bytes[cursor + 28], bytes[cursor + 29]]) as usize;
            let extra_len = u16::from_le_bytes([bytes[cursor + 30], bytes[cursor + 31]]) as usize;
            let comment_len = u16::from_le_bytes([bytes[cursor + 32], bytes[cursor + 33]]) as usize;
            let local_header_offset = u32::from_le_bytes([
                bytes[cursor + 42],
                bytes[cursor + 43],
                bytes[cursor + 44],
                bytes[cursor + 45],
            ]) as usize;

            let name_start = cursor + 46;
            if name_start + name_len <= bytes.len() {
                if let Ok(filename) = std::str::from_utf8(&bytes[name_start..name_start + name_len]) {
                    if filename == "word/document.xml" || filename == "word/document2.xml" {
                        if local_header_offset + 30 <= bytes.len() && &bytes[local_header_offset..local_header_offset + 4] == b"PK\x03\x04" {
                            let loc_name_len = u16::from_le_bytes([bytes[local_header_offset + 26], bytes[local_header_offset + 27]]) as usize;
                            let loc_extra_len = u16::from_le_bytes([bytes[local_header_offset + 28], bytes[local_header_offset + 29]]) as usize;
                            let data_start = local_header_offset + 30 + loc_name_len + loc_extra_len;

                            if data_start + comp_size <= bytes.len() {
                                let comp_data = &bytes[data_start..data_start + comp_size];
                                let xml_bytes = if comp_method == 8 {
                                    miniz_oxide::inflate::decompress_to_vec(comp_data).ok()
                                } else if comp_method == 0 {
                                    Some(comp_data.to_vec())
                                } else {
                                    None
                                };

                                if let Some(xml_raw) = xml_bytes {
                                    if let Ok(xml_str) = std::str::from_utf8(&xml_raw) {
                                        return parse_word_xml_text(xml_str);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            cursor += 46 + name_len + extra_len + comment_len;
        } else {
            cursor += 1;
        }
    }

    String::new()
}

/// Extrai texto de arquivos Word legados (.doc binário do Word 97-2003)
fn extract_text_from_legacy_doc(bytes: &[u8]) -> String {
    let mut out = String::new();
    let mut utf16_chunk = Vec::new();
    let mut i = 0;
    while i + 1 < bytes.len() {
        let b1 = bytes[i];
        let b2 = bytes[i + 1];
        if b2 == 0 && ((b1 >= 32 && b1 <= 126) || b1 == b'\n' || b1 == b'\r' || b1 == b'\t') {
            utf16_chunk.push(b1 as u16);
            i += 2;
        } else {
            if utf16_chunk.len() >= 4 {
                if let Ok(s) = String::from_utf16(&utf16_chunk) {
                    out.push_str(&s);
                    out.push(' ');
                }
            }
            utf16_chunk.clear();
            i += 1;
        }
    }
    if utf16_chunk.len() >= 4 {
        if let Ok(s) = String::from_utf16(&utf16_chunk) {
            out.push_str(&s);
        }
    }
    if out.trim().len() > 10 {
        return out;
    }
    extract_ascii_strings_fallback(bytes)
}

/// Extrai conteúdo de texto das tags <w:t> do Word XML, respeitando parágrafos
fn parse_word_xml_text(xml: &str) -> String {
    let mut out = String::new();
    let mut cursor = 0;
    while cursor < xml.len() {
        if let Some(pos) = xml[cursor..].find("<w:t") {
            let tag_start = cursor + pos;
            if let Some(tag_end) = xml[tag_start..].find('>') {
                let content_start = tag_start + tag_end + 1;
                if let Some(close_tag) = xml[content_start..].find("</w:t>") {
                    let content = &xml[content_start..content_start + close_tag];
                    out.push_str(content);
                    cursor = content_start + close_tag + 6;

                    if let Some(p_close) = xml[cursor..].find("</w:p>") {
                        if p_close < 30 {
                            out.push('\n');
                        } else {
                            out.push(' ');
                        }
                    } else {
                        out.push(' ');
                    }
                    continue;
                }
            }
            cursor = tag_start + 4;
        } else {
            break;
        }
    }
    out
}

/// Fallback para sequências de caracteres imprimíveis
fn extract_ascii_strings_fallback(bytes: &[u8]) -> String {
    let mut out = String::new();
    let mut chunk = Vec::new();
    for &b in bytes {
        if (b >= 32 && b <= 126) || b == b'\n' || b == b'\r' || b == b'\t' {
            chunk.push(b);
        } else {
            if chunk.len() >= 3 {
                if let Ok(s) = std::str::from_utf8(&chunk) {
                    out.push_str(s);
                    out.push(' ');
                }
            }
            chunk.clear();
        }
    }
    if chunk.len() >= 3 {
        if let Ok(s) = std::str::from_utf8(&chunk) {
            out.push_str(s);
        }
    }
    out
}

/// Varre pastas recursivamente coletando arquivos suportados
fn collect_files_recursively(dir: &Path, results: &mut Vec<PathBuf>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_files_recursively(&path, results);
            } else if path.is_file() {
                let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
                if ext != "exe" && ext != "dll" && ext != "obj" && ext != "bin" && ext != "git" {
                    results.push(path);
                }
            }
        }
    }
}