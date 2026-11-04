# Validador Financeiro em Lote (Rust 2021)

Sistema de alta performance para validação e auditoria antifraude de:
- **Boletos Bancários de Cobrança e Concessionárias (47D, 48D e 44D)**: Módulo 10 e Módulo 11 (DVs de bloco e DV Geral).
- **PIX Copia e Cola / EMV QR Code (BR Code)**: Validação estrutural TLV e integridade com Checksum **CRC16 CCITT-FALSE**.
- **Análise em Lote de Pastas**: Varre pastas inteiras de documentos (PDFs, DOCX, TXT) emitindo relatório consolidado.

---

## Como Executar

### 1. Modo Automático (analisa a pasta padrão './documentos'):
```cmd
cargo run
```

### 2. Analisar um Documento Específico (PDF, DOCX ou TXT):
```cmd
cargo run -- "meu_boleto.pdf"
```
Ou com valor esperado para checagem cruzada:
```cmd
cargo run -- "meu_boleto.pdf" 150.00
```

### 3. Analisar Vários Documentos de uma Pasta em Lote:
```cmd
cargo run -- "./minha_pasta_de_faturas"
```

### 4. Executar os Testes Unitários de Integridade:
```cmd
cargo test
```
Todos os 6 testes unitários com Payloads oficiais Febraban e Bacen passarão com 100% de sucesso.
