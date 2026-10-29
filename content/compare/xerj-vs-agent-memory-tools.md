---
title: "XERJ vs Mem0, Zep, Letta and Cognee for agent memory"
h1: "XERJ vs agent memory tools: Mem0, Zep, Letta and Cognee"
description: "How XERJ compares with Mem0, Zep, Letta and Cognee for AI agent memory: what each one stores, what it needs to run, and when to pick each one."
slug: "xerj-vs-agent-memory-tools"
cluster: "Comparison: agent memory"
question: "Should I use Mem0, Zep, Letta, Cognee or XERJ for AI agent memory?"
intent: "comparison"
published: "2026-10-28"
updated: "2026-10-28"
author: "XERJ documentation team"
reviewer: "XERJ engineering team"
schema_type: "Article"
structured_data: [faq]
agent_prompt: "Act as a coding agent. Read https://xerj.org/llms.txt, install XERJ, start a local node, store three facts about this project with POST /_memory/project, recall each one with POST /_memory/project/_recall, and tell me whether plain search recall is enough for my agent before I add an LLM-based memory tool."
answer: "Should I use Mem0, Zep, Letta, Cognee or XERJ for agent memory? Pick Mem0 or Cognee to have a model extract facts and links for you, Zep or Graphiti for facts that change over time, Letta for a full stateful agent. Pick XERJ for plain local memory, searched like any other data, with no model on the write path."
intent_group: "agent memory"
queries:
  - "mem0 vs zep vs letta"
  - "mem0 alternative open source"
  - "zep alternative self-hosted"
  - "agent memory tools comparison"
  - "self-hosted agent memory no LLM"
evidence:
  - claim: "Mem0 requires an LLM to function, with gpt-5-mini from OpenAI as the default, and uses OpenAI text-embedding-3-small as the default embedder. It ships as a library, a self-hosted server and a cloud platform, under Apache 2.0."
    source: "https://github.com/mem0ai/mem0"
  - claim: "Zep Cloud is the managed product. Zep Community Edition is deprecated and unsupported. Graphiti is the open-source framework inside Zep for temporal context graphs, and you supply the graph database, LLM and embedder."
    source: "https://github.com/getzep/zep"
  - claim: "Graphiti is an Apache-2.0 framework for building temporal context graphs for AI agents; you bring your own graph database such as Neo4j or FalkorDB."
    source: "https://github.com/getzep/graphiti"
  - claim: "Letta (formerly MemGPT) is a harness for stateful agents with memory blocks and a git-versioned memory filesystem (MemFS). The local runtime keeps agent state on the device. Apache 2.0."
    source: "https://github.com/letta-ai/letta-code"
  - claim: "Letta docs describe MemFS as the git-versioned memory filesystem agents read, write and reorganize."
    source: "https://docs.letta.com/llms.txt"
  - claim: "Cognee is an Apache-2.0 open-source AI memory platform that turns documents, code and conversations into a graph of entities and relationships plus searchable chunks. It runs locally without an LLM key using a local GLiNER extraction model; an LLM key adds generated answers."
    source: "https://github.com/topoteretes/cognee"
  - claim: "XERJ memory: POST /_memory/{namespace} stores the text the agent sends, recall defaults to BM25, no model runs on the write path, and each namespace is its own index."
    source: "engine/crates/xerj-api/src/memory_api.rs"
commands:
  - cmd: "curl -fsSL https://xerj.org/get | sh"
    note: "Install the single XERJ binary."
  - cmd: "curl -s -XPOST 'http://127.0.0.1:9200/_memory/project' -H 'content-type: application/json' -d '{\"text\":\"Releases are cut from the main branch on Fridays.\"}'"
    note: "Store one fact exactly as the agent wrote it."
  - cmd: "curl -s -XPOST 'http://127.0.0.1:9200/_memory/project/_recall' -H 'content-type: application/json' -d '{\"query\":\"when do we release\",\"k\":3}'"
    note: "Recall it. Plain query recall is BM25 over the stored text."
links_out:
  - "xerj-vs-vector-database"
  - "xerj-vs-khoj"
  - "answers/what-is-agentic-memory"
  - "answers/what-is-an-mcp-memory-server"
  - "answers/agentic-memory-vs-rag"
  - "answers/store-agent-memory-without-llm-call"
faq:
  - q: "What is the main difference between XERJ and Mem0?"
    a: "Mem0 uses an LLM to extract facts from conversations, and its README says it requires one. XERJ stores the text your agent sends as is and finds it with search, with no model on the write path."
  - q: "Is Zep open source?"
    a: "Zep Cloud is a commercial cloud product. Zep Community Edition is deprecated. The open-source part is Graphiti, an Apache-2.0 framework where you bring your own graph database, LLM and embedder."
  - q: "Is Letta a memory layer?"
    a: "Letta is broader: it is a harness for stateful agents, with memory blocks and a git-versioned memory filesystem. You run your agent inside Letta rather than calling it as a store."
  - q: "Does Cognee need an LLM?"
    a: "Not for ingestion and retrieval. Its README says it runs locally with a GLiNER model and a local embedder, and an LLM key adds generated answers."
  - q: "When is XERJ the better fit?"
    a: "When you want memory as plain searchable records on your own machine, next to the files and data your agent already searches, without an LLM or a second database."
  - q: "Did you benchmark these tools against each other?"
    a: "No. No head-to-head was run. This page compares what each project documents about itself."
---

**TL;DR**: Mem0, Zep, Letta and Cognee add intelligence on top of memory: models that extract facts, graphs that track change, or a whole agent runtime. XERJ does less on purpose. It stores what your agent writes and finds it with search, in one local binary with no model on the write path. No head-to-head was run.

## What each one is

| tool | what it is | needs a model to store? | license |
| --- | --- | --- | --- |
| Mem0 | memory layer that extracts facts with an LLM | yes, OpenAI by default | Apache 2.0 |
| Zep and Graphiti | Zep Cloud, plus the open Graphiti framework for temporal context graphs | Graphiti: you supply the LLM | Graphiti: Apache 2.0 |
| Letta | harness for stateful agents with memory blocks and MemFS | runs your agent's model | Apache 2.0 |
| Cognee | memory platform that builds a graph of entities and relationships | no; a local GLiNER model by default | Apache 2.0 |
| XERJ | search engine for AI agents with a memory API | no | Apache 2.0 |

Each row comes from the project's own README, linked in the sources.

## The real difference

The four tools above transform what the agent says before they store it. Mem0 extracts facts. Graphiti builds a graph whose facts have validity windows. Cognee pulls out entities and links. Letta lets the agent rewrite its own memory files.

XERJ keeps the raw text the agent sends and makes it searchable. That is less clever, and it is also cheaper and easier to inspect. Recent research points the same way: two 2026 studies found plain retrieval over raw history often matches more complex memory systems. See [agentic memory vs RAG](/answers/agentic-memory-vs-rag).

## When to choose Mem0 instead

Choose Mem0 when you want a model to turn chats into short facts. Expect an LLM call on every write. Mem0 also offers a cloud platform. XERJ memory runs on your own machine.

## When to choose Zep or Graphiti instead

Choose Zep Cloud when you want Zep to run it for you. Choose self-hosted Graphiti when facts change over time and you must know what was true when. Graphiti expects a graph store such as Neo4j or FalkorDB.

## When to choose Letta instead

Choose Letta when you want the whole agent, not just a store. It puts memory blocks, a memory filesystem, skills and channels in one harness.

## When to choose Cognee instead

Choose Cognee when you want documents, code and chats turned into a graph of entities and relationships. Cognee is a Python stack.

## When to choose XERJ

Choose XERJ when you want memory and search in one place, on your machine, with nothing extra to run.

- Store with `POST /_memory/{namespace}` and recall with `_recall`. Over MCP, use `xerj_memory_store` and `xerj_memory_recall`.
- Each namespace is its own index, so agents keep private memories.
- The same binary also searches your files, logs and code, so memory and data sit side by side.

## Limits you inherit with XERJ

XERJ is single-node: no replication and no failover. It does not extract facts or build entity links for you. The default embedder is lexical feature hashing, so `semantic: true` recall matches shared words unless you start the node with `--embed-mode neural`. Memories are permanent until explicitly forgotten.

## What was not measured

No head-to-head was run. No accuracy, speed or cost numbers are claimed for any tool here. Every competitor fact comes from that project's own README or docs, checked on 2026-10-28.
