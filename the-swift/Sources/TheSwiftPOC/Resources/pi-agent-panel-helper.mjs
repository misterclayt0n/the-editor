import { spawnSync } from "node:child_process";
import fs from "node:fs/promises";
import path from "node:path";
import readline from "node:readline";
import { pathToFileURL } from "node:url";

function debug(...args) {
  if (process.env.PI_AGENT_PANEL_DEBUG === "1") {
    process.stderr.write(`[agent-panel-helper] ${args.join(" ")}\n`);
  }
}

function send(message) {
  process.stdout.write(`${JSON.stringify(message)}\n`);
}

function errorToObject(error) {
  if (error instanceof Error) {
    return { message: error.message, stack: error.stack };
  }
  return { message: String(error) };
}

function withHome(p) {
  if (!p.startsWith("~/")) return p;
  return path.join(process.env.HOME || "", p.slice(2));
}

function candidatePiModulePaths() {
  return [
    process.env.PI_CODING_AGENT_DIST,
    "~/.nvm/versions/node/v24.13.0/lib/node_modules/@mariozechner/pi-coding-agent/dist/index.js",
    "/opt/homebrew/lib/node_modules/@mariozechner/pi-coding-agent/dist/index.js",
    "/usr/local/lib/node_modules/@mariozechner/pi-coding-agent/dist/index.js",
  ]
    .filter(Boolean)
    .map(withHome);
}

async function resolveExistingPath(candidates) {
  for (const candidate of candidates) {
    try {
      await fs.access(candidate);
      return candidate;
    } catch {
      // continue
    }
  }
  throw new Error(`No existing path found. Checked: ${candidates.join(", ")}`);
}

const piModulePath = await resolveExistingPath(candidatePiModulePaths());
const piModule = await import(pathToFileURL(piModulePath).href);
const typeBoxPath = process.env.PI_TYPEBOX_DIST
  ? withHome(process.env.PI_TYPEBOX_DIST)
  : path.join(path.dirname(path.dirname(piModulePath)), "node_modules", "@sinclair", "typebox", "build", "esm", "index.mjs");
const { Type } = await import(pathToFileURL(typeBoxPath).href);

const {
  createAgentSession,
  SessionManager,
} = piModule;

let currentAgentDir = process.env.PI_AGENT_DIR || path.join(process.env.HOME || "", ".pi", "agent");
let currentCwd = process.cwd();
let pendingRequests = new Map();
let nextRequestId = 1;
const sessionRecords = new Map();
const sessionMutationTails = new Map();

function sessionPathForSession(session) {
  return session.sessionFile || session.sessionManager?.getSessionFile?.() || null;
}

function sessionRecordKey(sessionPath) {
  return String(sessionPath || "").trim();
}

async function withSessionMutationLock(sessionPath, task) {
  const key = sessionRecordKey(sessionPath);
  const previousTail = sessionMutationTails.get(key) || Promise.resolve();
  let releaseCurrentTail;
  const currentTail = new Promise((resolve) => {
    releaseCurrentTail = resolve;
  });
  const nextTail = previousTail.then(() => currentTail);
  sessionMutationTails.set(key, nextTail);
  await previousTail;
  try {
    return await task();
  } finally {
    releaseCurrentTail?.();
    if (sessionMutationTails.get(key) === nextTail) {
      sessionMutationTails.delete(key);
    }
  }
}

function requestApp(method, params = {}) {
  const id = `helper-${nextRequestId++}`;
  send({ type: "request", id, method, params });
  return new Promise((resolve, reject) => {
    pendingRequests.set(id, { resolve, reject, method });
  });
}

function resolvePendingResponse(id, result, error) {
  const pending = pendingRequests.get(id);
  if (!pending) return false;
  pendingRequests.delete(id);
  if (error) {
    pending.reject(new Error(error.message || String(error)));
  } else {
    pending.resolve(result);
  }
  return true;
}

function extractText(content) {
  if (typeof content === "string") return content;
  if (!Array.isArray(content)) return "";
  return content
    .filter((part) => part && part.type === "text" && typeof part.text === "string")
    .map((part) => part.text)
    .join("");
}

function extractAssistantText(message) {
  if (!message || message.role !== "assistant" || !Array.isArray(message.content)) return "";
  return message.content
    .filter((part) => part && part.type === "text" && typeof part.text === "string")
    .map((part) => part.text)
    .join("");
}

function pathSummary(filePath) {
  if (!filePath || typeof filePath !== "string") return "";
  const normalized = filePath.replace(/^file:\/\//, "");
  const parts = normalized.split(path.sep).filter(Boolean);
  if (parts.length <= 4) return normalized;
  return `…/${parts.slice(-4).join("/")}`;
}

function summarizeToolInput(toolName, input) {
  switch (toolName) {
    case "read":
      return `Read ${pathSummary(input?.path)}`;
    case "edit":
      return `Edit ${pathSummary(input?.path)}`;
    case "write":
      return `Write ${pathSummary(input?.path)}`;
    case "bash": {
      const command = typeof input?.command === "string" ? input.command.trim() : "";
      return command ? `Run ${command.split("\n")[0].slice(0, 80)}` : "Run command";
    }
    default:
      return toolName;
  }
}

function encodeToolInputJSON(input) {
  if (input == null) return null;
  try {
    return JSON.stringify(input, null, 2);
  } catch {
    return null;
  }
}

function stripEditorContextWrap(raw) {
  const marker = "[User request]\n";
  const idx = raw.indexOf(marker);
  if (!raw.startsWith("[Editor context]") || idx < 0) {
    return { text: raw, contextSummary: null };
  }
  const header = raw.slice(0, idx);
  const body = raw.slice(idx + marker.length);
  let contextSummary = null;
  const activeMatch = header.match(/^Active file:\s*(.+)$/m);
  if (activeMatch) {
    const base = activeMatch[1].trim().split("/").pop();
    if (base) contextSummary = base;
  }
  return { text: body, contextSummary };
}

function buildInlineRewritePrompt({
  filePath,
  sourceLabel,
  language,
  lineStart,
  lineEnd,
  selectionText,
  prompt,
}) {
  const location = (() => {
    if (lineStart && lineEnd && lineEnd >= lineStart) {
      return lineStart === lineEnd ? `:${lineStart}` : `:${lineStart}-${lineEnd}`;
    }
    return "";
  })();
  const languageLabel = language ? `Language: ${language}` : "Language: unknown";
  const fileLabel = filePath || sourceLabel || "selection";
  return [
    "You are performing an inline rewrite of a selected piece of text from an editor.",
    "Rewrite ONLY the contents inside <rewrite_this> according to the user prompt.",
    "Return ONLY the rewritten text.",
    "Do not include markdown fences.",
    "Do not include explanations.",
    "Do not mention the file or summarize your changes.",
    "",
    `File: ${fileLabel}${location}`,
    languageLabel,
    "",
    "<prompt>",
    prompt,
    "</prompt>",
    "",
    "<rewrite_this>",
    selectionText,
    "</rewrite_this>",
  ].join("\n");
}

function normalizeInlineRewriteResponse(text) {
  let normalized = String(text || "").trim();
  if (!normalized) return "";

  if (normalized.startsWith("<rewrite_this>") && normalized.endsWith("</rewrite_this>")) {
    normalized = normalized
      .replace(/^<rewrite_this>\s*/i, "")
      .replace(/\s*<\/rewrite_this>$/i, "")
      .trim();
  }

  const fencedMatch = normalized.match(/^```[^\n]*\n([\s\S]*?)\n```$/);
  if (fencedMatch) {
    normalized = fencedMatch[1].trim();
  }

  return normalized;
}

function historyItemsFromMessages(messages) {
  const toolResultsByCallId = new Map();
  const knownToolCallIds = new Set();

  for (const message of messages) {
    if (message.role === "assistant" && Array.isArray(message.content)) {
      for (const part of message.content) {
        if (part?.type === "toolCall" && part.id) {
          knownToolCallIds.add(part.id);
        }
      }
    }
    if (message.role === "toolResult" && message.toolCallId) {
      toolResultsByCallId.set(message.toolCallId, message);
    }
  }

  return messages.flatMap((message, index) => {
    const id = `${message.role}-${message.timestamp ?? index}-${index}`;
    switch (message.role) {
      case "user": {
        const text = extractText(message.content);
        if (!text) return [];
        const stripped = stripEditorContextWrap(text);
        return [{ id, kind: "user", text: stripped.text, context: stripped.contextSummary ?? undefined }];
      }
      case "assistant": {
        const rows = [];
        const runtimeAssistantId = `assistant-${message.timestamp ?? index}`;
        if (Array.isArray(message.content)) {
          for (const [partIndex, part] of message.content.entries()) {
            if (part?.type === "text") {
              const text = typeof part.text === "string" ? part.text : "";
              if (text) {
                rows.push({
                  id: `${id}-text-${partIndex}`,
                  correlationID: runtimeAssistantId,
                  kind: "assistant",
                  text,
                  isStreaming: false,
                });
              }
              continue;
            }
            if (part?.type === "thinking") {
              const text = typeof part.thinking === "string" ? part.thinking : "";
              if (text) {
                rows.push({
                  id: `${id}-thinking-${partIndex}`,
                  correlationID: `${runtimeAssistantId}-thinking-${partIndex}`,
                  kind: "thinking",
                  text,
                  isStreaming: false,
                });
              }
              continue;
            }
            if (part?.type !== "toolCall" || !part.id) continue;
            const toolName = typeof part.name === "string" ? part.name : "tool";
            const input = part.arguments ?? null;
            const result = toolResultsByCallId.get(part.id);
            const text = extractText(result?.content);
            let detailsText = text;
            if (!detailsText && result?.details?.diff) {
              detailsText = result.details.diff;
            }
            rows.push({
              id: part.id,
              correlationID: part.id,
              kind: "tool",
              title: summarizeToolInput(toolName, input),
              text: detailsText || "",
              isStreaming: false,
              status: result?.isError ? "failed" : "done",
              context: toolName,
              toolInputJSON: encodeToolInputJSON(input),
            });
          }
        }
        if (message.stopReason === "error" || message.stopReason === "aborted") {
          rows.push({
            id: `${id}-terminal`,
            correlationID: `${runtimeAssistantId}-terminal`,
            kind: "note",
            text: message.errorMessage || (message.stopReason === "aborted" ? "Operation aborted" : "Error"),
            isStreaming: false,
          });
        }
        return rows;
      }
      case "toolResult": {
        if (message.toolCallId && knownToolCallIds.has(message.toolCallId)) {
          return [];
        }
        const text = extractText(message.content);
        let detailsText = text;
        if (!detailsText && message.details?.diff) {
          detailsText = message.details.diff;
        }
        if (!detailsText) return [];
        return [{
          id: message.toolCallId || id,
          kind: "tool",
          title: summarizeToolInput(message.toolName || "tool", null),
          text: detailsText,
          isStreaming: false,
          status: message.isError ? "failed" : "done",
          context: message.toolName || undefined,
        }];
      }
      case "custom": {
        if (message.display === false) return [];
        const text = extractText(message.content);
        return text ? [{ id, kind: "note", text }] : [];
      }
      case "branchSummary": {
        return message.summary ? [{ id, kind: "note", text: message.summary, noteStyle: "branchSummary" }] : [];
      }
      case "compactionSummary": {
        return message.summary
          ? [{ id, kind: "note", text: message.summary, noteStyle: "compactionSummary", tokensBefore: message.tokensBefore ?? null }]
          : [];
      }
      default:
        return [];
    }
  });
}

function slashCommandsForSession(session) {
  const builtInCommands = [
    { name: "settings", description: "Open settings menu", source: "builtin" },
    { name: "model", description: "Select model", source: "builtin" },
    { name: "scoped-models", description: "Enable or disable models for cycling", source: "builtin" },
    { name: "export", description: "Export session", source: "builtin" },
    { name: "import", description: "Import and resume a session", source: "builtin" },
    { name: "share", description: "Share session as a secret GitHub gist", source: "builtin" },
    { name: "copy", description: "Copy last agent message to clipboard", source: "builtin" },
    { name: "name", description: "Set session display name", source: "builtin" },
    { name: "session", description: "Show session info and stats", source: "builtin" },
    { name: "changelog", description: "Show changelog entries", source: "builtin" },
    { name: "hotkeys", description: "Show keyboard shortcuts", source: "builtin" },
    { name: "fork", description: "Create a fork from a previous message", source: "builtin" },
    { name: "tree", description: "Navigate session tree", source: "builtin" },
    { name: "login", description: "Login with OAuth provider", source: "builtin" },
    { name: "logout", description: "Logout from OAuth provider", source: "builtin" },
    { name: "new", description: "Start a new session", source: "builtin" },
    { name: "compact", description: "Manually compact the session context", source: "builtin" },
    { name: "resume", description: "Resume a different session", source: "builtin" },
    { name: "reload", description: "Reload extensions, skills, prompts, and themes", source: "builtin" },
    { name: "quit", description: "Quit pi", source: "builtin" },
  ];

  const builtInNames = new Set(builtInCommands.map((command) => command.name));

  const promptTemplates = session.promptTemplates.map((template) => ({
    name: template.name,
    description: template.description || "",
    source: "prompt",
  }));

  const extensionCommands = session.extensionRunner
    ? session.extensionRunner
        .getRegisteredCommands()
        .filter((command) => !builtInNames.has(command.invocationName))
        .map((command) => ({
          name: command.invocationName,
          description: command.description || "",
          source: "extension",
        }))
    : [];

  const skills = session.resourceLoader.getSkills().skills.map((skill) => ({
    name: `skill:${skill.name}`,
    description: skill.description || "",
    source: "skill",
  }));

  return [...builtInCommands, ...promptTemplates, ...extensionCommands, ...skills];
}

function contextUsagePayload(session) {
  const usage = session.getContextUsage?.();
  if (!usage) return null;
  return {
    tokens: usage.tokens ?? null,
    percent: usage.percent ?? null,
    contextWindow: usage.contextWindow ?? null,
  };
}

function formatFooterTokens(count) {
  if (!Number.isFinite(count) || count <= 0) return "0";
  if (count < 1000) return `${count}`;
  if (count < 10000) return `${(count / 1000).toFixed(1)}k`;
  if (count < 1000000) return `${Math.round(count / 1000)}k`;
  if (count < 10000000) return `${(count / 1000000).toFixed(1)}M`;
  return `${Math.round(count / 1000000)}M`;
}

function resolveGitBranchSync(cwd) {
  try {
    const result = spawnSync("git", ["--no-optional-locks", "symbolic-ref", "--quiet", "--short", "HEAD"], {
      cwd,
      encoding: "utf8",
      stdio: ["ignore", "pipe", "ignore"],
    });
    const branch = result.status === 0 ? result.stdout.trim() : "";
    return branch || null;
  } catch {
    return null;
  }
}

function footerPayload(session) {
  const cwd = session.sessionManager?.getCwd?.() || currentCwd || process.cwd();
  const contextUsage = session.getContextUsage?.();

  let totalInput = 0;
  let totalOutput = 0;
  let totalCacheRead = 0;
  let totalCacheWrite = 0;
  let totalCost = 0;

  for (const entry of session.sessionManager?.getEntries?.() || []) {
    if (entry.type === "message" && entry.message?.role === "assistant") {
      totalInput += entry.message.usage?.input ?? 0;
      totalOutput += entry.message.usage?.output ?? 0;
      totalCacheRead += entry.message.usage?.cacheRead ?? 0;
      totalCacheWrite += entry.message.usage?.cacheWrite ?? 0;
      totalCost += entry.message.usage?.cost?.total ?? 0;
    }
  }

  const sessionName = session.sessionManager?.getSessionName?.() || session.sessionName || null;
  session.modelRegistry.refresh();
  const availableModels = session.modelRegistry.getAvailable();
  const availableProviderCount = new Set(availableModels.map((model) => model.provider)).size;

  return {
    cwd,
    gitBranch: resolveGitBranchSync(cwd),
    sessionName,
    totalInput,
    totalOutput,
    totalCacheRead,
    totalCacheWrite,
    totalCost,
    usingSubscription: session.model ? session.modelRegistry.isUsingOAuth(session.model) : false,
    contextTokens: contextUsage?.tokens ?? null,
    contextPercent: contextUsage?.percent ?? null,
    contextWindow: contextUsage?.contextWindow ?? session.model?.contextWindow ?? null,
    autoCompactEnabled: !!session.autoCompactionEnabled,
    modelProvider: session.model?.provider ?? null,
    modelID: session.model?.id ?? null,
    modelName: session.model?.name ?? session.model?.id ?? null,
    modelSupportsReasoning: !!session.model?.reasoning,
    thinkingLevel: session.thinkingLevel ?? "off",
    availableProviderCount,
    formatted: {
      input: totalInput > 0 ? formatFooterTokens(totalInput) : null,
      output: totalOutput > 0 ? formatFooterTokens(totalOutput) : null,
      cacheRead: totalCacheRead > 0 ? formatFooterTokens(totalCacheRead) : null,
      cacheWrite: totalCacheWrite > 0 ? formatFooterTokens(totalCacheWrite) : null,
      contextWindow: (contextUsage?.contextWindow ?? session.model?.contextWindow)
        ? formatFooterTokens(contextUsage?.contextWindow ?? session.model?.contextWindow)
        : null,
    },
  };
}

function mapAvailableModels(session) {
  session.modelRegistry.refresh();
  const currentRef = session.model ? `${session.model.provider}/${session.model.id}` : null;
  return session.modelRegistry.getAvailable().map((model) => ({
    provider: model.provider,
    id: model.id,
    name: model.name || model.id,
    reference: `${model.provider}/${model.id}`,
    isCurrent: currentRef === `${model.provider}/${model.id}`,
    supportsReasoning: !!model.reasoning,
  }));
}

function sortModelPayload(lhs, rhs) {
  if (lhs.isCurrent && !rhs.isCurrent) return -1;
  if (!lhs.isCurrent && rhs.isCurrent) return 1;
  const providerCompare = lhs.provider.localeCompare(rhs.provider);
  if (providerCompare !== 0) return providerCompare;
  return lhs.id.localeCompare(rhs.id);
}

function isAllowedInlineRewriteModel(model) {
  const provider = String(model.provider || "").toLowerCase();
  const id = String(model.id || "").toLowerCase();
  const name = String(model.name || "").toLowerCase();

  if (provider === "opencode-go") {
    return id.includes("kimi") || name.includes("kimi");
  }

  if (provider === "cursor" || provider === "cursor-agent") {
    return id.includes("composer") || name.includes("composer");
  }

  return false;
}

function availableModelsPayload(session) {
  return mapAvailableModels(session).sort(sortModelPayload);
}

function availableInlineRewriteModelsPayload(session) {
  return mapAvailableModels(session)
    .filter(isAllowedInlineRewriteModel)
    .sort(sortModelPayload);
}

function sessionMetadataPayload(session, { includeModels = false } = {}) {
  return {
    sessionName: session.sessionName || null,
    model: session.model ? `${session.model.provider}/${session.model.id}` : null,
    thinkingLevel: session.thinkingLevel,
    contextUsage: contextUsagePayload(session),
    footer: footerPayload(session),
    ...(includeModels ? { models: availableModelsPayload(session) } : {}),
  };
}

function sessionSnapshot(session) {
  const sessionPath = sessionPathForSession(session);
  const models = availableModelsPayload(session);
  return {
    sessionId: session.sessionId,
    sessionPath,
    sessionFile: sessionPath,
    sessionName: session.sessionName || null,
    model: session.model ? `${session.model.provider}/${session.model.id}` : null,
    thinkingLevel: session.thinkingLevel,
    history: historyItemsFromMessages(session.messages),
    commands: slashCommandsForSession(session),
    contextUsage: contextUsagePayload(session),
    models,
    footer: footerPayload(session),
  };
}

function emitSessionStatus(record) {
  send({
    type: "event",
    sessionPath: record.sessionPath,
    event: "session_status",
    payload: {
      model: record.session.model ? `${record.session.model.provider}/${record.session.model.id}` : null,
      contextUsage: contextUsagePayload(record.session),
      footer: footerPayload(record.session),
    },
  });
}

async function bindSessionRecord(record) {
  if (record.unsubscribe) {
    record.unsubscribe();
    record.unsubscribe = null;
  }

  await record.session.bindExtensions({});

  record.unsubscribe = record.session.subscribe((event) => {
    try {
      handleSessionEvent(record, event);
    } catch (error) {
      send({
        type: "event",
        sessionPath: record.sessionPath,
        event: "runtime_error",
        payload: errorToObject(error),
      });
    }
  });
}

function emitAssistantCompletion(record, message, fallbackId) {
  if (!message || message.role !== "assistant") return;
  const assistantId = record.currentAssistantMessageId || fallbackId || `assistant-${message.timestamp ?? Date.now()}`;
  if (record.currentAssistantCompletionEmitted) return;
  send({
    type: "event",
    sessionPath: record.sessionPath,
    event: "assistant_completed",
    payload: {
      id: assistantId,
      text: extractAssistantText(message),
      stopReason: message.stopReason || null,
      errorMessage: message.errorMessage || null,
    },
  });
  record.currentAssistantCompletionEmitted = true;
}

async function waitForLiveSessionSettle(record, timeoutMs = 750) {
  const startedAt = Date.now();
  while (Date.now() - startedAt < timeoutMs) {
    if (!record.currentAssistantMessageId) {
      await new Promise((resolve) => setTimeout(resolve, 25));
      return;
    }
    await new Promise((resolve) => setTimeout(resolve, 10));
  }
}

function handleSessionEvent(record, event) {
  switch (event.type) {
    case "message_start": {
      if (event.message.role === "user") {
        if (record.suppressNextUserMessageStart) {
          record.suppressNextUserMessageStart = false;
          break;
        }
        const text = extractText(event.message.content);
        if (text) {
          send({
            type: "event",
            sessionPath: record.sessionPath,
            event: "user_message",
            payload: { id: `${event.message.timestamp}`, text },
          });
        }
      }
      if (event.message.role === "assistant") {
        record.currentAssistantMessageId = `assistant-${event.message.timestamp ?? Date.now()}`;
        record.currentAssistantCompletionEmitted = false;
      }
      break;
    }
    case "message_update": {
      if (event.assistantMessageEvent.type === "text_delta") {
        if (!record.currentAssistantMessageId) {
          record.currentAssistantMessageId = `assistant-${Date.now()}`;
        }
        send({
          type: "event",
          sessionPath: record.sessionPath,
          event: "assistant_delta",
          payload: { id: record.currentAssistantMessageId, delta: event.assistantMessageEvent.delta },
        });
      } else if (event.assistantMessageEvent.type === "done") {
        emitAssistantCompletion(
          record,
          event.assistantMessageEvent.message,
          record.currentAssistantMessageId || `assistant-${event.assistantMessageEvent.message?.timestamp ?? Date.now()}`,
        );
      } else if (event.assistantMessageEvent.type === "error") {
        emitAssistantCompletion(
          record,
          event.assistantMessageEvent.error,
          record.currentAssistantMessageId || `assistant-${event.assistantMessageEvent.error?.timestamp ?? Date.now()}`,
        );
      } else if (event.assistantMessageEvent.type === "thinking_start") {
        if (!record.currentAssistantMessageId) {
          record.currentAssistantMessageId = `assistant-${Date.now()}`;
        }
        send({
          type: "event",
          sessionPath: record.sessionPath,
          event: "thinking_started",
          payload: { id: `${record.currentAssistantMessageId}-thinking-${event.assistantMessageEvent.contentIndex}` },
        });
      } else if (event.assistantMessageEvent.type === "thinking_delta") {
        if (!record.currentAssistantMessageId) {
          record.currentAssistantMessageId = `assistant-${Date.now()}`;
        }
        send({
          type: "event",
          sessionPath: record.sessionPath,
          event: "thinking_delta",
          payload: {
            id: `${record.currentAssistantMessageId}-thinking-${event.assistantMessageEvent.contentIndex}`,
            delta: event.assistantMessageEvent.delta,
          },
        });
      } else if (event.assistantMessageEvent.type === "thinking_end") {
        if (!record.currentAssistantMessageId) {
          record.currentAssistantMessageId = `assistant-${Date.now()}`;
        }
        send({
          type: "event",
          sessionPath: record.sessionPath,
          event: "thinking_completed",
          payload: {
            id: `${record.currentAssistantMessageId}-thinking-${event.assistantMessageEvent.contentIndex}`,
            text: event.assistantMessageEvent.content || "",
          },
        });
      }
      break;
    }
    case "message_end": {
      if (event.message.role === "assistant") {
        if (Array.isArray(event.message.content)) {
          for (const [partIndex, part] of event.message.content.entries()) {
            if (part?.type === "thinking" && typeof part.thinking === "string" && part.thinking) {
              send({
                type: "event",
                sessionPath: record.sessionPath,
                event: "thinking_completed",
                payload: {
                  id: `${record.currentAssistantMessageId || `assistant-${event.message.timestamp ?? Date.now()}`}-thinking-${partIndex}`,
                  text: part.thinking,
                },
              });
            }
          }
        }
        emitAssistantCompletion(
          record,
          event.message,
          record.currentAssistantMessageId || `assistant-${event.message.timestamp ?? Date.now()}`,
        );
        record.currentAssistantMessageId = null;
      } else if (event.message.role === "custom" && event.message.display !== false) {
        const text = extractText(event.message.content);
        if (text) {
          send({
            type: "event",
            sessionPath: record.sessionPath,
            event: "note_message",
            payload: { id: `${event.message.timestamp}`, text },
          });
        }
      } else if (event.message.role === "branchSummary" && event.message.summary) {
        send({
          type: "event",
          sessionPath: record.sessionPath,
          event: "note_message",
          payload: {
            id: `${event.message.timestamp}`,
            text: event.message.summary,
            noteStyle: "branchSummary",
          },
        });
      }
      break;
    }
    case "tool_execution_start": {
      send({
        type: "event",
        sessionPath: record.sessionPath,
        event: "tool_started",
        payload: {
          id: event.toolCallId,
          toolName: event.toolName,
          summary: summarizeToolInput(event.toolName, event.args),
          inputJSON: encodeToolInputJSON(event.args),
        },
      });
      break;
    }
    case "tool_execution_update": {
      const text = extractText(event.partialResult?.content);
      if (text) {
        send({
          type: "event",
          sessionPath: record.sessionPath,
          event: "tool_updated",
          payload: { id: event.toolCallId, text },
        });
      }
      break;
    }
    case "tool_execution_end": {
      const text = extractText(event.result?.content);
      let detailsText = text;
      if (!detailsText && event.result?.details?.diff) {
        detailsText = event.result.details.diff;
      }
      send({
        type: "event",
        sessionPath: record.sessionPath,
        event: "tool_completed",
        payload: {
          id: event.toolCallId,
          toolName: event.toolName,
          isError: event.isError,
          text: detailsText || "",
          summary: summarizeToolInput(event.toolName, event.args),
          inputJSON: encodeToolInputJSON(event.args),
        },
      });
      break;
    }
    case "compaction_start": {
      send({
        type: "event",
        sessionPath: record.sessionPath,
        event: "compaction_start",
        payload: {
          reason: event.reason,
        },
      });
      break;
    }
    case "compaction_end": {
      const payload = {
        reason: event.reason,
        aborted: Boolean(event.aborted),
        willRetry: Boolean(event.willRetry),
        errorMessage: event.errorMessage || null,
      };
      if (!event.aborted && event.result) {
        payload.snapshot = sessionSnapshot(record.session);
      }
      send({
        type: "event",
        sessionPath: record.sessionPath,
        event: "compaction_end",
        payload,
      });
      emitSessionStatus(record);
      break;
    }
    case "agent_end": {
      emitSessionStatus(record);
      break;
    }
    default:
      break;
  }
}

function normalizeEditorPath(inputPath, cwd) {
  if (path.isAbsolute(inputPath)) return inputPath;
  return path.resolve(cwd, inputPath);
}

function diffSummary(before, after, filePath) {
  const beforeLines = before.split(/\r?\n/);
  const afterLines = after.split(/\r?\n/);
  let changed = 0;
  const max = Math.max(beforeLines.length, afterLines.length);
  for (let index = 0; index < max; index += 1) {
    if ((beforeLines[index] || "") !== (afterLines[index] || "")) changed += 1;
  }
  return [`--- ${filePath}`, `+++ ${filePath}`, `Changed lines: ${changed}`].join("\n");
}

function truncateText(text, maxLines = 2000, maxBytes = 50 * 1024) {
  const lines = text.split(/\r?\n/);
  let resultLines = lines;
  let truncated = false;
  if (lines.length > maxLines) {
    resultLines = lines.slice(0, maxLines);
    truncated = true;
  }
  let result = resultLines.join("\n");
  const byteLength = Buffer.byteLength(result, "utf8");
  if (byteLength > maxBytes) {
    result = Buffer.from(result, "utf8").subarray(0, maxBytes).toString("utf8");
    truncated = true;
  }
  if (truncated) {
    result += "\n\n[output truncated in agent panel v0]";
  }
  return result;
}

function defaultSessionDirForCwd(cwd) {
  const safePath = `--${cwd.replace(/^[/\\]/, "").replace(/[/\\:]/g, "-")}--`;
  return path.join(currentAgentDir, "sessions", safePath);
}

function truncatePreviewText(text, maxLength = 280) {
  const flattened = String(text || "").replace(/\s+/g, " ").trim();
  if (!flattened) {
    return "";
  }
  if (flattened.length <= maxLength) {
    return flattened;
  }
  return `${flattened.slice(0, maxLength - 1)}…`;
}

async function readRecentSessionSummary(filePath, modified) {
  let fileHandle;
  try {
    fileHandle = await fs.open(filePath, "r");
    const buffer = Buffer.alloc(128 * 1024);
    const { bytesRead } = await fileHandle.read(buffer, 0, buffer.length, 0);
    const prefix = buffer.toString("utf8", 0, bytesRead);
    const lines = prefix.split(/\r?\n/);

    let sessionId = path.basename(filePath, path.extname(filePath));
    let name = null;
    let firstMessage = "";

    for (const line of lines) {
      if (!line.trim()) {
        continue;
      }

      let entry;
      try {
        entry = JSON.parse(line);
      } catch {
        continue;
      }

      if (entry?.type === "session" && typeof entry.id === "string" && entry.id.trim()) {
        sessionId = entry.id.trim();
        continue;
      }

      if (entry?.type === "session_info" && typeof entry.name === "string") {
        const trimmedName = entry.name.trim();
        if (trimmedName) {
          name = trimmedName;
        }
        continue;
      }

      if (entry?.type === "message" && entry.message?.role === "user") {
        const text = truncatePreviewText(extractText(entry.message.content));
        if (text) {
          firstMessage = text;
          break;
        }
      }
    }

    return {
      id: sessionId,
      path: filePath,
      name,
      firstMessage: firstMessage || "(no messages)",
      modified: modified.toISOString(),
    };
  } catch {
    return null;
  } finally {
    try {
      await fileHandle?.close();
    } catch {
      // ignore
    }
  }
}

async function listRecentSessionSummaries(cwd, limit = 20) {
  const sessionDir = defaultSessionDirForCwd(cwd);
  try {
    const entries = await fs.readdir(sessionDir, { withFileTypes: true });
    const candidates = await Promise.all(
      entries
        .filter((entry) => entry.isFile() && entry.name.endsWith(".jsonl"))
        .map(async (entry) => {
          const filePath = path.join(sessionDir, entry.name);
          try {
            const stats = await fs.stat(filePath);
            return { filePath, modified: stats.mtime };
          } catch {
            return null;
          }
        }),
    );

    const sortedCandidates = candidates
      .filter(Boolean)
      .sort((lhs, rhs) => rhs.modified.getTime() - lhs.modified.getTime());

    const sessions = [];
    for (const candidate of sortedCandidates) {
      if (sessions.length >= limit) {
        break;
      }
      const summary = await readRecentSessionSummary(candidate.filePath, candidate.modified);
      if (summary) {
        sessions.push(summary);
      }
    }

    return sessions;
  } catch {
    return [];
  }
}

function createEditorBackedTools(cwd, sessionPathProvider) {
  const replaceEditSchema = Type.Object(
    {
      oldText: Type.String(),
      newText: Type.String(),
    },
    { additionalProperties: false },
  );

  function emitAgentFollow(payload) {
    send({
      type: "event",
      sessionPath: sessionPathProvider?.() || null,
      event: "agent_follow",
      payload,
    });
  }

  function normalizedLine(value) {
    if (!Number.isFinite(value)) return null;
    return Math.max(1, Math.trunc(value));
  }

  return [
    {
      name: "read",
      label: "read",
      description: "Read a file through the editor bridge.",
      promptSnippet: "Read file contents through the editor bridge",
      parameters: Type.Object(
        {
          path: Type.String(),
          offset: Type.Optional(Type.Number()),
          limit: Type.Optional(Type.Number()),
        },
        { additionalProperties: false },
      ),
      async execute(_toolCallId, input) {
        const absolutePath = normalizeEditorPath(input.path, cwd);
        const lineStart = normalizedLine(input.offset);
        const lineEnd = lineStart != null && Number.isFinite(input.limit)
          ? Math.max(lineStart, lineStart + Math.max(Math.trunc(input.limit), 1) - 1)
          : null;
        emitAgentFollow({
          kind: "read",
          phase: "before",
          path: absolutePath,
          lineStart,
          lineEnd,
          summary: `Reading ${pathSummary(absolutePath)}`,
        });
        const result = await requestApp("editor.readFile", {
          path: absolutePath,
          offset: input.offset ?? null,
          limit: input.limit ?? null,
        });
        let text = String(result.text ?? "");
        const lines = text.split(/\r?\n/);
        const offset = Math.max(1, Number.isFinite(input.offset) ? input.offset : 1);
        const start = Math.max(offset - 1, 0);
        const end = input.limit ? start + Math.max(input.limit, 0) : undefined;
        text = lines.slice(start, end).join("\n");
        text = truncateText(text);
        return {
          content: [{ type: "text", text }],
          details: {
            path: absolutePath,
            source: result.source || "disk",
          },
        };
      },
    },
    {
      name: "write",
      label: "write",
      description: "Write a file through the editor bridge.",
      promptSnippet: "Write files through the editor bridge",
      parameters: Type.Object(
        {
          path: Type.String(),
          content: Type.String(),
        },
        { additionalProperties: false },
      ),
      async execute(_toolCallId, input) {
        const absolutePath = normalizeEditorPath(input.path, cwd);
        emitAgentFollow({
          kind: "write",
          phase: "before",
          path: absolutePath,
          lineStart: null,
          lineEnd: null,
          summary: `Writing ${pathSummary(absolutePath)}`,
        });
        const result = await requestApp("editor.writeFileAnimated", {
          sessionPath: sessionPathProvider?.() || null,
          path: absolutePath,
          content: input.content,
        });
        const lineCount = input.content.length === 0 ? 1 : input.content.split(/\r?\n/).length;
        emitAgentFollow({
          kind: "write",
          phase: "after",
          path: absolutePath,
          lineStart: 1,
          lineEnd: Math.min(lineCount, 12),
          summary: `Wrote ${pathSummary(absolutePath)}`,
        });
        return {
          content: [{ type: "text", text: `Wrote ${pathSummary(absolutePath)}` }],
          details: {
            path: absolutePath,
            bytes: result.bytes ?? Buffer.byteLength(input.content, "utf8"),
            diff: result.diff || "",
          },
        };
      },
    },
    {
      name: "edit",
      label: "edit",
      description: "Apply precise file edits through the editor bridge.",
      promptSnippet: "Edit files through the editor bridge using exact replacements",
      parameters: Type.Object(
        {
          path: Type.String(),
          edits: Type.Array(replaceEditSchema),
        },
        { additionalProperties: false },
      ),
      async execute(_toolCallId, input) {
        const absolutePath = normalizeEditorPath(input.path, cwd);
        const preview = await requestApp("editor.previewEdit", {
          path: absolutePath,
          edits: input.edits,
        });
        emitAgentFollow({
          kind: "edit",
          phase: "before",
          path: absolutePath,
          lineStart: normalizedLine(preview.firstChangedLine),
          lineEnd: normalizedLine(preview.firstChangedLine),
          summary: `Editing ${pathSummary(absolutePath)}`,
        });
        const result = await requestApp("editor.editFileAnimated", {
          sessionPath: sessionPathProvider?.() || null,
          path: absolutePath,
          edits: input.edits,
        });
        const firstChangedLine = normalizedLine(result.firstChangedLine ?? preview.firstChangedLine);
        emitAgentFollow({
          kind: "edit",
          phase: "after",
          path: absolutePath,
          lineStart: firstChangedLine,
          lineEnd: firstChangedLine,
          summary: `Edited ${pathSummary(absolutePath)}`,
        });
        return {
          content: [{ type: "text", text: `Edited ${pathSummary(absolutePath)}` }],
          details: {
            path: absolutePath,
            diff: result.diff || "",
            firstChangedLine: result.firstChangedLine ?? null,
          },
        };
      },
    },
  ];
}

async function createSessionRecord({ cwd, sessionManager }) {
  currentCwd = cwd;
  let sessionPath = null;
  const { session } = await createAgentSession({
    cwd,
    agentDir: currentAgentDir,
    sessionManager,
    customTools: createEditorBackedTools(cwd, () => sessionPath),
  });
  sessionPath = sessionPathForSession(session);
  if (!sessionPath) {
    throw new Error("Could not determine persisted session path.");
  }
  const record = {
    cwd,
    session,
    sessionPath,
    unsubscribe: null,
    currentAssistantMessageId: null,
    currentAssistantCompletionEmitted: false,
    suppressNextUserMessageStart: false,
  };
  sessionRecords.set(sessionPath, record);
  await bindSessionRecord(record);
  return record;
}

async function getSessionRecord(sessionPath) {
  const key = sessionRecordKey(sessionPath);
  if (!key) {
    throw new Error("A session path is required.");
  }
  const existing = sessionRecords.get(key);
  if (existing) {
    return existing;
  }
  const sessionManager = SessionManager.open(key);
  return await createSessionRecord({
    cwd: sessionManager.getCwd(),
    sessionManager,
  });
}

async function closeSessionRecord(sessionPath) {
  const key = sessionRecordKey(sessionPath);
  if (!key) return;
  const record = sessionRecords.get(key);
  if (!record) return;
  record.unsubscribe?.();
  record.unsubscribe = null;
  try {
    await record.session.abort();
  } catch {
    // ignore
  }
  record.session.dispose();
  sessionRecords.delete(key);
}

async function handleHelperRequest(id, method, params) {
  try {
    switch (method) {
      case "createSession": {
        const cwd = params?.cwd ? String(params.cwd) : process.cwd();
        const record = await createSessionRecord({
          cwd,
          sessionManager: SessionManager.create(cwd),
        });
        send({ type: "response", id, result: sessionSnapshot(record.session) });
        return;
      }
      case "getSessionSnapshot": {
        const sessionPath = String(params?.sessionPath ?? "");
        const record = await getSessionRecord(sessionPath);
        send({ type: "response", id, result: sessionSnapshot(record.session) });
        return;
      }
      case "openSession": {
        const sessionPath = String(params?.path ?? params?.sessionPath ?? "");
        const record = await getSessionRecord(sessionPath);
        send({ type: "response", id, result: sessionSnapshot(record.session) });
        return;
      }
      case "closeSession": {
        const sessionPath = String(params?.sessionPath ?? "");
        await withSessionMutationLock(sessionPath, async () => {
          await closeSessionRecord(sessionPath);
        });
        send({ type: "response", id, result: { closed: true } });
        return;
      }
      case "prompt": {
        const sessionPath = String(params?.sessionPath ?? "");
        const record = await getSessionRecord(sessionPath);
        const rawText = String(params?.text ?? "");
        record.suppressNextUserMessageStart = true;
        send({
          type: "event",
          sessionPath: record.sessionPath,
          event: "user_message",
          payload: { id: `user-${Date.now()}`, text: rawText },
        });
        await withSessionMutationLock(record.sessionPath, async () => {
          await record.session.prompt(rawText);
        });
        await waitForLiveSessionSettle(record);
        emitSessionStatus(record);
        send({
          type: "response",
          id,
          result: {
            prompted: true,
            sessionPath: record.sessionPath,
          },
        });
        return;
      }
      case "abort": {
        const sessionPath = String(params?.sessionPath ?? "");
        const record = await getSessionRecord(sessionPath);
        await record.session.abort();
        emitSessionStatus(record);
        send({ type: "response", id, result: { aborted: true } });
        return;
      }
      case "listSessions": {
        const cwd = params?.cwd ? String(params.cwd) : currentCwd;
        const sessions = await listRecentSessionSummaries(cwd);
        send({
          type: "response",
          id,
          result: sessions.map((session) => ({
            id: session.id,
            path: session.path,
            name: session.name || null,
            firstMessage: stripEditorContextWrap(session.firstMessage || "").text,
            modified: session.modified,
          })),
        });
        return;
      }
      case "listModels": {
        const sessionPath = String(params?.sessionPath ?? "");
        const record = await getSessionRecord(sessionPath);
        send({ type: "response", id, result: availableModelsPayload(record.session) });
        return;
      }
      case "listInlineRewriteModels": {
        const cwd = params?.cwd ? String(params.cwd) : currentCwd;
        const sessionManager = SessionManager.inMemory(cwd);
        const { session } = await createAgentSession({
          cwd,
          agentDir: currentAgentDir,
          sessionManager,
          tools: [],
          customTools: [],
          thinkingLevel: "off",
        });
        try {
          send({ type: "response", id, result: availableInlineRewriteModelsPayload(session) });
        } finally {
          session.dispose();
        }
        return;
      }
      case "setModel": {
        const sessionPath = String(params?.sessionPath ?? "");
        const record = await getSessionRecord(sessionPath);
        const provider = String(params?.provider ?? "").trim();
        const modelId = String(params?.modelId ?? "").trim();
        await withSessionMutationLock(record.sessionPath, async () => {
          record.session.modelRegistry.refresh();
          const model = record.session.modelRegistry
            .getAvailable()
            .find((candidate) => candidate.provider === provider && candidate.id === modelId);
          if (!model) {
            throw new Error(`Model not found: ${provider}/${modelId}`);
          }
          await record.session.setModel(model);
        });
        emitSessionStatus(record);
        send({ type: "response", id, result: sessionMetadataPayload(record.session, { includeModels: true }) });
        return;
      }
      case "compact": {
        const sessionPath = String(params?.sessionPath ?? "");
        const record = await getSessionRecord(sessionPath);
        const customInstructions = typeof params?.customInstructions === "string"
          ? params.customInstructions.trim() || undefined
          : undefined;
        await withSessionMutationLock(record.sessionPath, async () => {
          await record.session.compact(customInstructions);
        });
        emitSessionStatus(record);
        send({ type: "response", id, result: sessionSnapshot(record.session) });
        return;
      }
      case "cycleThinkingLevel": {
        const sessionPath = String(params?.sessionPath ?? "");
        const record = await getSessionRecord(sessionPath);
        await withSessionMutationLock(record.sessionPath, async () => {
          record.session.cycleThinkingLevel();
        });
        emitSessionStatus(record);
        send({ type: "response", id, result: sessionMetadataPayload(record.session, { includeModels: true }) });
        return;
      }
      case "setSessionName": {
        const sessionPath = String(params?.sessionPath ?? "");
        const record = await getSessionRecord(sessionPath);
        const name = String(params?.name ?? "").trim();
        if (!name) {
          throw new Error("Session name cannot be empty");
        }
        await withSessionMutationLock(record.sessionPath, async () => {
          record.session.setSessionName(name);
        });
        emitSessionStatus(record);
        send({ type: "response", id, result: sessionMetadataPayload(record.session) });
        return;
      }
      case "getContextUsage": {
        const sessionPath = String(params?.sessionPath ?? "");
        const record = await getSessionRecord(sessionPath);
        send({ type: "response", id, result: contextUsagePayload(record.session) });
        return;
      }
      case "inlineRewrite": {
        const cwd = params?.cwd ? String(params.cwd) : currentCwd;
        const sourceLabel = String(params?.sourceLabel ?? params?.filePath ?? "selection");
        const filePath = typeof params?.filePath === "string" && params.filePath.trim().length > 0
          ? String(params.filePath)
          : null;
        const language = typeof params?.language === "string" && params.language.trim().length > 0
          ? String(params.language)
          : null;
        const lineStart = Number.isFinite(params?.lineStart) ? Number(params.lineStart) : null;
        const lineEnd = Number.isFinite(params?.lineEnd) ? Number(params.lineEnd) : null;
        const selectionText = String(params?.selectionText ?? "");
        const prompt = String(params?.prompt ?? "").trim();
        const provider = typeof params?.provider === "string" ? params.provider.trim() : "";
        const modelId = typeof params?.modelId === "string" ? params.modelId.trim() : "";
        const modelSourceSessionPath = typeof params?.modelSourceSessionPath === "string"
          ? params.modelSourceSessionPath.trim()
          : "";
        const thinkingLevel = typeof params?.thinkingLevel === "string" && params.thinkingLevel.trim().length > 0
          ? params.thinkingLevel.trim()
          : "off";

        if (selectionText.length === 0) {
          throw new Error("Selection text is required for inline rewrite.");
        }
        if (!prompt) {
          throw new Error("Prompt is required for inline rewrite.");
        }

        debug(
          "inlineRewrite.begin",
          `cwd=${cwd}`,
          `file=${filePath || "-"}`,
          `lines=${lineStart ?? "-"}-${lineEnd ?? "-"}`,
          `selectionChars=${selectionText.length}`,
          `promptChars=${prompt.length}`,
          `requestedModel=${provider && modelId ? `${provider}/${modelId}` : "nil"}`,
          `modelSourceSessionPath=${modelSourceSessionPath || "nil"}`,
          `thinkingLevel=${thinkingLevel}`,
        );

        const sourceRecord = modelSourceSessionPath
          ? await getSessionRecord(modelSourceSessionPath)
          : null;
        debug(
          "inlineRewrite.sourceRecord",
          `hasSourceRecord=${sourceRecord ? "yes" : "no"}`,
          `sourceSession=${sourceRecord?.session ? "yes" : "no"}`,
        );
        const sessionManager = SessionManager.inMemory(cwd);
        const { session } = await createAgentSession({
          cwd,
          agentDir: currentAgentDir,
          sessionManager,
          modelRegistry: sourceRecord?.session?.modelRegistry,
          tools: [],
          customTools: [],
          thinkingLevel,
        });

        try {
          session.modelRegistry.refresh();
          const availableInlineModels = session.modelRegistry
            .getAvailable()
            .filter(isAllowedInlineRewriteModel);
          debug(
            "inlineRewrite.models",
            `count=${availableInlineModels.length}`,
            `models=${availableInlineModels.map((model) => `${model.provider}/${model.id}`).join(",") || "-"}`,
          );
          const selectedModel = (provider && modelId)
            ? availableInlineModels.find((candidate) => candidate.provider === provider && candidate.id === modelId)
            : null;
          const fallbackModel = availableInlineModels[0] ?? null;
          const model = selectedModel ?? fallbackModel;
          if (!model) {
            throw new Error("No inline rewrite models are configured. Add a Kimi model via opencode-go or a Composer model via Cursor.");
          }
          debug(
            "inlineRewrite.modelSelected",
            `requested=${provider && modelId ? `${provider}/${modelId}` : "nil"}`,
            `resolved=${model.provider}/${model.id}`,
            `matched=${selectedModel ? "yes" : "no"}`,
            `fallback=${fallbackModel ? `${fallbackModel.provider}/${fallbackModel.id}` : "nil"}`,
          );
          await session.setModel(model);

          const rewritePrompt = buildInlineRewritePrompt({
            filePath,
            sourceLabel,
            language,
            lineStart,
            lineEnd,
            selectionText,
            prompt,
          });
          debug("inlineRewrite.prompt.start", `rewritePromptChars=${rewritePrompt.length}`);
          await session.prompt(rewritePrompt);
          debug("inlineRewrite.prompt.end", `messageCount=${session.messages.length}`);
          const assistantMessage = [...session.messages].reverse().find((message) => message.role === "assistant");
          const rewrittenText = normalizeInlineRewriteResponse(extractAssistantText(assistantMessage));
          debug(
            "inlineRewrite.response",
            `assistantFound=${assistantMessage ? "yes" : "no"}`,
            `rewrittenChars=${rewrittenText.length}`,
          );
          if (!rewrittenText) {
            throw new Error("Inline rewrite returned no text.");
          }
          debug(
            "inlineRewrite.response.send",
            `id=${id}`,
            `textChars=${rewrittenText.length}`,
            `model=${session.model ? `${session.model.provider}/${session.model.id}` : "nil"}`,
          );
          send({
            type: "response",
            id,
            result: {
              text: rewrittenText,
              model: session.model ? `${session.model.provider}/${session.model.id}` : null,
            },
          });
        } finally {
          debug("inlineRewrite.cleanup.begin");
          try {
            await session.abort();
          } catch {
            // ignore
          }
          session.dispose();
          debug("inlineRewrite.cleanup.end");
        }
        return;
      }
      default:
        throw new Error(`Unknown method: ${method}`);
    }
  } catch (error) {
    send({ type: "response", id, error: errorToObject(error) });
  }
}

const rl = readline.createInterface({
  input: process.stdin,
  crlfDelay: Infinity,
});

rl.on("line", async (line) => {
  if (!line.trim()) return;
  let message;
  try {
    message = JSON.parse(line);
  } catch (error) {
    send({ type: "event", event: "runtime_error", payload: { message: `Invalid JSON from app: ${line}`, error: errorToObject(error) } });
    return;
  }

  if (message.type === "response") {
    resolvePendingResponse(message.id, message.result, message.error);
    return;
  }

  if (message.type === "request") {
    await handleHelperRequest(message.id, message.method, message.params || {});
  }
});

process.on("SIGINT", async () => {
  for (const record of sessionRecords.values()) {
    try {
      record.unsubscribe?.();
      await record.session.abort();
    } catch {
      // ignore
    }
    try {
      record.session.dispose();
    } catch {
      // ignore
    }
  }
  sessionRecords.clear();
  process.exit(0);
});
