# Klock OpenRouter LangChain Demo

This directory contains three demonstration scripts that showcase Klock's ability to prevent race conditions and deadlocks in multi-agent environments using LangChain and OpenRouter.

## Setup

1. **Start the Klock Server** (Required):
   ```bash
   # From the repository root
   cargo run --release -p klock-cli -- serve
   ```

2. **Install Dependencies**:
   ```bash
   pip install -r requirements.txt
   # Also ensure the local klock and klock-langchain packages are installed or in your PYTHONPATH
   ```

3. **Set your API Key**:
   ```bash
   export OPENROUTER_API_KEY="your_api_key_here"
   ```

## Included Demos

- **`demo.py`**: A simple two-agent conflict resolution scenario.
- **`demo_scale.py`**: A larger benchark with 5 agents and 3 files.
- **`compare_algorithms.py`**: A mathematical comparison of Wait-Die vs other concurrency models.

## Why Klock?
Without Klock, autonomous agents writing to the same files will cause **silent data loss**. Standard locks often cause **deadlocks** where agents freeze forever. Klock's **Wait-Die** algorithm ensures data integrity without freezing your agents.
