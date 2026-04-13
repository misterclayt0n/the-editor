import { createEditToolDefinition, createReadToolDefinition, createWriteToolDefinition, type ExtensionAPI, withFileMutationQueue } from "@mariozechner/pi-coding-agent";
import { access as fsAccess, mkdir as fsMkdir, readFile as fsReadFile, stat as fsStat, writeFile as fsWriteFile } from "node:fs/promises";
import net from "node:net";
import path from "node:path";
import { constants } from "node:fs";

const STATUS_KEY = "the-editor";
const GIT_MANIFEST_RELATIVE_PATH = path.join("the-editor", "pi-bridge.json");
const MAX_BUFFERED_LINES = 1000;
const EXTENSION_INSTANCE_KEY = "__the_editor_pi_extension_loaded__";
const STREAM_EDIT_DELAY_MS = 16;
const STREAM_EDIT_MAX_CHANGED_CODEPOINTS = 12000;
const STREAM_EDIT_MAX_CHUNKS = 128;
const STREAM_EDIT_TARGET_CHUNK_CODEPOINTS = 220;
const STREAM_EDIT_MAX_CHUNK_CODEPOINTS = 320;
const STREAM_EDIT_TARGET_CHUNK_LINES = 12;
const BRIDGE_RPC_TIMEOUT_MS = 5000;
const PI_BRIDGE_DEBUG = process.env.THE_EDITOR_PI_BRIDGE_DEBUG === "1";

type JsonValue = null | boolean | number | string | JsonValue[] | { [key: string]: JsonValue };
type BridgeState = "detached" | "attached" | "busy";
type ExtensionOwnerRecord = {
	token: object;
};

type PiBridgeManifest = {
	version: number;
	transport: string;
	workspaceRoot: string;
	socketPath: string;
	editorPid: number;
};

type SelectionPayload = {
	absolutePath: string;
	workspaceRelativePath: string;
	language?: string;
	selectedText: string;
	startChar: number;
	endChar: number;
	startLine: number;
	endLine: number;
};

type ReadFileResponse = {
	path: string;
	content: string;
	fromLiveBuffer: boolean;
	openedBuffer: boolean;
};

type WriteFileResponse = {
	path: string;
	saved: boolean;
	openedBuffer: boolean;
};

type ReplaceRangeResponse = {
	path: string;
	saved: boolean;
	openedBuffer: boolean;
};

type AgentPresenceKind = "focus" | "read" | "edit_preview" | "edit_applied" | "write";

type AgentPresenceParams = {
	path: string;
	kind?: AgentPresenceKind;
	range?: {
		startChar: number;
		endChar: number;
	};
	startLine?: number;
	endLine?: number;
	cursorChar?: number;
};

type ReadFileOptions = {
	sendPresence?: boolean;
};

type AttachRejectedParams = {
	reason?: string;
};

type BridgeDebugEntry = {
	at: string;
	message: string;
};

type PiBridgeEnvelope =
	| {
			type: "request";
			id: string;
			method: string;
			params: JsonValue;
	  }
	| {
			type: "response";
			id: string;
			ok: boolean;
			result?: JsonValue;
			error?: string;
	  }
	| {
			type: "notification";
			method: string;
			params: JsonValue;
	  };

function formatStatus(theme: any, kind: BridgeState, text: string): string {
	if (kind === "attached" || kind === "busy") {
		return theme.fg("accent", text);
	}
	return theme.fg("dim", text);
}

function formatSelectionPrompt(selection: SelectionPayload): string {
	return selection.selectedText;
}

function isPathInside(root: string, candidate: string): boolean {
	const relative = path.relative(root, candidate);
	return relative === "" || (!relative.startsWith("..") && !path.isAbsolute(relative));
}

function isProbablyTextPath(filePath: string): boolean {
	const ext = path.extname(filePath).toLowerCase();
	return !new Set([
		".png",
		".jpg",
		".jpeg",
		".gif",
		".webp",
		".bmp",
		".ico",
		".pdf",
		".zip",
		".gz",
		".tar",
		".tgz",
		".bz2",
		".xz",
		".7z",
		".wasm",
		".so",
		".dylib",
		".dll",
		".o",
		".a",
		".bin",
		".mp3",
		".mp4",
		".mov",
		".avi",
		".mkv",
	]).has(ext);
}

function detectImageMimeTypeFromPath(filePath: string): string | null {
	switch (path.extname(filePath).toLowerCase()) {
		case ".png":
			return "image/png";
		case ".jpg":
		case ".jpeg":
			return "image/jpeg";
		case ".gif":
			return "image/gif";
		case ".webp":
			return "image/webp";
		default:
			return null;
	}
}

async function findBridgeManifest(startCwd: string): Promise<PiBridgeManifest | null> {
	let current = path.resolve(startCwd);
	while (true) {
		const manifest = await readBridgeManifest(path.join(current, ".git"), current);
		if (manifest) {
			return manifest;
		}
		const parent = path.dirname(current);
		if (parent === current) {
			return null;
		}
		current = parent;
	}
}

async function readBridgeManifest(gitMarkerOrManifestPath: string, workspaceRoot: string): Promise<PiBridgeManifest | null> {
	const manifestPath = await resolveManifestPath(gitMarkerOrManifestPath, workspaceRoot);
	if (!manifestPath) {
		return null;
	}
	try {
		const manifest = JSON.parse(await fsReadFile(manifestPath, "utf8")) as PiBridgeManifest;
		if (manifest.transport === "unix-jsonl" && manifest.socketPath && manifest.workspaceRoot) {
			return manifest;
		}
	} catch {
		// Keep walking.
	}
	return null;
}

async function resolveManifestPath(gitMarkerOrManifestPath: string, workspaceRoot: string): Promise<string | null> {
	if (path.basename(gitMarkerOrManifestPath) !== ".git") {
		return gitMarkerOrManifestPath;
	}
	try {
		await fsAccess(gitMarkerOrManifestPath, constants.F_OK);
		const stats = await fsStat(gitMarkerOrManifestPath);
		if (stats.isDirectory()) {
			return path.join(gitMarkerOrManifestPath, GIT_MANIFEST_RELATIVE_PATH);
		}
		if (stats.isFile()) {
			const gitdirText = await fsReadFile(gitMarkerOrManifestPath, "utf8");
			const match = gitdirText.match(/^gitdir:\s*(.+)\s*$/m);
			if (!match) {
				return null;
			}
			const gitDir = path.resolve(workspaceRoot, match[1]);
			return path.join(gitDir, GIT_MANIFEST_RELATIVE_PATH);
		}
	} catch {
		return null;
	}
	return null;
}

function sleep(ms: number): Promise<void> {
	return new Promise((resolve) => setTimeout(resolve, ms));
}

function formatDebugPath(filePath: string): string {
	return path.basename(filePath) || filePath;
}

function resolveToolPath(cwd: string, filePath: string): string {
	return path.isAbsolute(filePath) ? filePath : path.resolve(cwd, filePath);
}

function stripBom(text: string): { bom: string; text: string } {
	return text.startsWith("\uFEFF") ? { bom: "\uFEFF", text: text.slice(1) } : { bom: "", text };
}

function detectLineEnding(text: string): "\n" | "\r\n" {
	return text.includes("\r\n") ? "\r\n" : "\n";
}

function normalizeToLf(text: string): string {
	return text.replace(/\r\n/g, "\n");
}

function restoreLineEndings(text: string, lineEnding: "\n" | "\r\n"): string {
	return lineEnding === "\r\n" ? text.replace(/\n/g, "\r\n") : text;
}

function collectExactMatchIndices(content: string, needle: string): number[] {
	const matches: number[] = [];
	if (needle.length === 0) {
		return matches;
	}
	let index = content.indexOf(needle);
	while (index !== -1) {
		matches.push(index);
		index = content.indexOf(needle, index + needle.length);
	}
	return matches;
}

function applyExactEditsLocally(
	rawContent: string,
	edits: Array<{ oldText: string; newText: string }>,
	filePath: string,
): string {
	const { bom, text } = stripBom(rawContent);
	const lineEnding = detectLineEnding(text);
	const normalizedContent = normalizeToLf(text);
	const replacements = edits.map((edit, index) => {
		const oldText = normalizeToLf(edit.oldText);
		const newText = normalizeToLf(edit.newText);
		if (oldText.length === 0) {
			throw new Error(`Could not find edits[${index}] in ${filePath}. The oldText must match exactly including all whitespace and newlines.`);
		}
		const matches = collectExactMatchIndices(normalizedContent, oldText);
		if (matches.length === 0) {
			throw new Error(`Could not find edits[${index}] in ${filePath}. The oldText must match exactly including all whitespace and newlines.`);
		}
		if (matches.length > 1) {
			throw new Error(`edits[${index}] matched multiple regions in ${filePath}. The oldText must be unique in the file.`);
		}
		return {
			index,
			start: matches[0],
			end: matches[0] + oldText.length,
			newText,
		};
	});

	replacements.sort((left, right) => left.start - right.start);
	for (let index = 1; index < replacements.length; index++) {
		if (replacements[index - 1].end > replacements[index].start) {
			throw new Error(`edits[${replacements[index].index}] overlaps another edit in ${filePath}. Merge nearby changes into one edit.`);
		}
	}

	let cursor = 0;
	let nextContent = "";
	for (const replacement of replacements) {
		nextContent += normalizedContent.slice(cursor, replacement.start);
		nextContent += replacement.newText;
		cursor = replacement.end;
	}
	nextContent += normalizedContent.slice(cursor);
	return bom + restoreLineEndings(nextContent, lineEnding);
}

function describeError(error: unknown): string {
	if (error instanceof Error) {
		return error.message;
	}
	return String(error);
}

function isMissingFileError(error: unknown): boolean {
	return typeof error === "object" && error !== null && "code" in error && (error as { code?: string }).code === "ENOENT";
}

function countCodePoints(text: string): number {
	return Array.from(text).length;
}

function splitTextByCodePoints(text: string, chunkSize: number): string[] {
	const chars = Array.from(text);
	const chunks: string[] = [];
	for (let index = 0; index < chars.length; index += chunkSize) {
		chunks.push(chars.slice(index, index + chunkSize).join(""));
	}
	return chunks;
}

function lineNumberToCharIndex(text: string, lineNumber: number): number {
	const clampedLineNumber = Math.max(1, lineNumber);
	let currentLine = 1;
	let index = 0;
	for (const ch of text) {
		if (currentLine >= clampedLineNumber) {
			break;
		}
		index += 1;
		if (ch === "\n") {
			currentLine += 1;
		}
	}
	return index;
}

function codePointIndexFromUtf16Index(text: string, utf16Index: number): number {
	return Array.from(text.slice(0, utf16Index)).length;
}

function readPresenceForInput(input: unknown): Pick<AgentPresenceParams, "startLine" | "endLine"> | null {
	if (typeof input !== "object" || input === null || !("path" in input)) {
		return null;
	}
	const maybeOffset = (input as { offset?: unknown }).offset;
	const maybeLimit = (input as { limit?: unknown }).limit;
	if (typeof maybeOffset !== "number" || !Number.isFinite(maybeOffset)) {
		return null;
	}
	const startLine = Math.max(1, Math.trunc(maybeOffset));
	const limit = typeof maybeLimit === "number" && Number.isFinite(maybeLimit)
		? Math.max(1, Math.trunc(maybeLimit))
		: 1;
	return {
		startLine,
		endLine: startLine + limit - 1,
	};
}

function exactEditUnionRange(
	content: string,
	edits: Array<{ oldText: string; newText: string }>,
): { startChar: number; endChar: number; cursorChar: number } {
	const normalizedContent = normalizeToLf(stripBom(content).text);
	const replacements = edits.map((edit, index) => {
		const oldText = normalizeToLf(edit.oldText);
		const newText = normalizeToLf(edit.newText);
		const matches = collectExactMatchIndices(normalizedContent, oldText);
		if (matches.length !== 1) {
			throw new Error(`Could not resolve edits[${index}] for agent follow.`);
		}
		const startChar = codePointIndexFromUtf16Index(normalizedContent, matches[0]);
		return {
			start: startChar,
			newEnd: startChar + countCodePoints(newText),
		};
	});
	replacements.sort((left, right) => left.start - right.start);
	const startChar = replacements[0]?.start ?? 0;
	const endChar = replacements.reduce((maxEnd, replacement) => Math.max(maxEnd, replacement.newEnd), startChar);
	const cursorChar = replacements[replacements.length - 1]?.newEnd ?? endChar;
	return { startChar, endChar, cursorChar };
}

function countLineBreaks(text: string): number {
	return (text.match(/\r\n|\n/gu) ?? []).length;
}

function splitLinesPreservingNewlines(text: string): string[] {
	return (text.match(/[^\r\n]*(?:\r\n|\n|$)/gu) ?? []).filter((part) => part.length > 0);
}

function tokenizeStreamingText(text: string): string[] {
	if (text.length === 0) {
		return [];
	}
	const chunks: string[] = [];
	let currentChunk = "";
	let currentCodePoints = 0;
	let currentLineBreaks = 0;

	const pushCurrentChunk = () => {
		if (currentChunk.length === 0) {
			return;
		}
		chunks.push(currentChunk);
		currentChunk = "";
		currentCodePoints = 0;
		currentLineBreaks = 0;
	};

	for (const line of splitLinesPreservingNewlines(text)) {
		const lineParts = countCodePoints(line) > STREAM_EDIT_MAX_CHUNK_CODEPOINTS
			? splitTextByCodePoints(line, STREAM_EDIT_MAX_CHUNK_CODEPOINTS)
			: [line];
		for (const part of lineParts) {
			const partCodePoints = countCodePoints(part);
			const partLineBreaks = countLineBreaks(part);
			const wouldOverflowCodePoints = currentCodePoints > 0
				&& currentCodePoints + partCodePoints > STREAM_EDIT_TARGET_CHUNK_CODEPOINTS;
			const wouldOverflowLines = currentLineBreaks > 0
				&& currentLineBreaks + partLineBreaks > STREAM_EDIT_TARGET_CHUNK_LINES;
			if (wouldOverflowCodePoints || wouldOverflowLines) {
				pushCurrentChunk();
			}
			currentChunk += part;
			currentCodePoints += partCodePoints;
			currentLineBreaks += partLineBreaks;
			if (
				currentCodePoints >= STREAM_EDIT_TARGET_CHUNK_CODEPOINTS
				|| currentLineBreaks >= STREAM_EDIT_TARGET_CHUNK_LINES
			) {
				pushCurrentChunk();
			}
		}
	}

	pushCurrentChunk();
	return chunks;
}

function sharedPrefixLength(currentChars: string[], targetChars: string[]): number {
	let index = 0;
	while (index < currentChars.length && index < targetChars.length && currentChars[index] === targetChars[index]) {
		index++;
	}
	return index;
}

function sharedSuffixLength(currentChars: string[], targetChars: string[], prefixLength: number): number {
	let suffixLength = 0;
	while (
		suffixLength < currentChars.length - prefixLength &&
		suffixLength < targetChars.length - prefixLength &&
		currentChars[currentChars.length - 1 - suffixLength] === targetChars[targetChars.length - 1 - suffixLength]
	) {
		suffixLength++;
	}
	return suffixLength;
}

function buildStreamingReplacePlan(currentContent: string, targetContent: string): {
	startChar: number;
	endChar: number;
	currentMiddle: string;
	targetMiddle: string;
} | null {
	if (currentContent === targetContent) {
		return null;
	}
	const currentChars = Array.from(currentContent);
	const targetChars = Array.from(targetContent);
	const prefixLength = sharedPrefixLength(currentChars, targetChars);
	const suffixLength = sharedSuffixLength(currentChars, targetChars, prefixLength);
	return {
		startChar: prefixLength,
		endChar: currentChars.length - suffixLength,
		currentMiddle: currentChars.slice(prefixLength, currentChars.length - suffixLength).join(""),
		targetMiddle: targetChars.slice(prefixLength, targetChars.length - suffixLength).join(""),
	};
}

function isBridgeTransportError(error: unknown): boolean {
	const message = describeError(error).toLowerCase();
	return (
		message.includes("editor bridge disconnected") ||
		message.includes("editor bridge reconnecting") ||
		message.includes("editor bridge is not attached") ||
		message.includes("socket hang up") ||
		message.includes("write after end") ||
		message.includes("broken pipe") ||
		message.includes("this socket has been ended by the other party") ||
		message.includes("err_stream_destroyed") ||
		message.includes("econnreset") ||
		message.includes("econnrefused") ||
		message.includes("enoent") ||
		message.includes("epipe")
	);
}

async function withBridgeFallback<T>(
	bridge: EditorBridgeClient,
	filePath: string,
	bridgeOperation: () => Promise<T>,
	fallbackOperation: () => Promise<T>,
): Promise<T> {
	if (!(await bridge.manifestForFilePath(filePath))) {
		return fallbackOperation();
	}
	try {
		return await bridgeOperation();
	} catch (error) {
		if (!isBridgeTransportError(error)) {
			throw error;
		}
		bridge.recordDebug(`bridge fallback for ${formatDebugPath(filePath)}: ${describeError(error)}`);
		return fallbackOperation();
	}
}

function editorBridgeRequiredMessage(filePath: string): string {
	return `The editor bridge must be attached for workspace edit/write operations (${formatDebugPath(filePath)}). These tool calls must go through the-editor transactions.`;
}

async function assertWorkspaceMutationBridgeReady(bridge: EditorBridgeClient, filePath: string): Promise<void> {
	if (!(await bridge.requiresWorkspaceMutationBridge(filePath))) {
		return;
	}
	await bridge.tryAutoAttachForMutation();
}

async function readFileWithBridgeFallback(
	bridge: EditorBridgeClient,
	absolutePath: string,
	source: string,
): Promise<Buffer> {
	const manifest = await bridge.manifestForFilePath(absolutePath);
	if (!manifest) {
		bridge.recordDebug(`${source}: read via fs fallback ${formatDebugPath(absolutePath)}`);
		return fsReadFile(absolutePath);
	}
	return withBridgeFallback(
		bridge,
		absolutePath,
		async () => {
			bridge.recordDebug(`${source}: read via bridge ${formatDebugPath(absolutePath)}`);
			const response = await bridge.readFile(absolutePath, { sendPresence: false });
			return Buffer.from(response.content, "utf8");
		},
		async () => {
			bridge.recordDebug(`${source}: read via fs fallback ${formatDebugPath(absolutePath)}`);
			return fsReadFile(absolutePath);
		},
	);
}

async function readTextFileForStreaming(
	bridge: EditorBridgeClient,
	absolutePath: string,
	source: string,
): Promise<{ content: string; openedBuffer: boolean; fromBridge: boolean }> {
	const manifest = await bridge.manifestForFilePath(absolutePath);
	try {
		if (manifest) {
			bridge.recordDebug(`${source}: stream baseline via bridge ${formatDebugPath(absolutePath)}`);
			const response = await bridge.readFile(absolutePath, { sendPresence: false });
			return { content: response.content, openedBuffer: response.openedBuffer, fromBridge: true };
		}
		bridge.recordDebug(`${source}: stream baseline via fs ${formatDebugPath(absolutePath)}`);
		return { content: await fsReadFile(absolutePath, "utf8"), openedBuffer: false, fromBridge: false };
	} catch (error) {
		if (isMissingFileError(error)) {
			return { content: "", openedBuffer: false, fromBridge: !!manifest };
		}
		throw error;
	}
}

async function streamWriteFileWithBridge(
	bridge: EditorBridgeClient,
	absolutePath: string,
	content: string,
	source: string,
): Promise<void> {
	const baseline = await readTextFileForStreaming(bridge, absolutePath, source);
	const currentContent = baseline.content;
	if (baseline.fromBridge && baseline.openedBuffer) {
		bridge.recordDebug(
			`${source}: stream baseline opened hidden buffer for ${formatDebugPath(absolutePath)} -> write_file`,
		);
		await bridge.writeFile(absolutePath, content);
		return;
	}
	const plan = buildStreamingReplacePlan(currentContent, content);
	if (!plan) {
		return;
	}

	const changedCodePoints = countCodePoints(plan.currentMiddle) + countCodePoints(plan.targetMiddle);
	if (changedCodePoints > STREAM_EDIT_MAX_CHANGED_CODEPOINTS) {
		bridge.recordDebug(
			`${source}: large diff -> write_file ${formatDebugPath(absolutePath)} (${changedCodePoints} codepoints)`,
		);
		await bridge.writeFile(absolutePath, content);
		return;
	}

	bridge.recordDebug(
		`${source}: streaming ${formatDebugPath(absolutePath)} (${changedCodePoints} codepoints changed start=${plan.startChar} end=${plan.endChar} currentMiddle=${countCodePoints(plan.currentMiddle)} targetMiddle=${countCodePoints(plan.targetMiddle)})`,
	);

	if (plan.targetMiddle.length === 0) {
		const deletionChunks = tokenizeStreamingText(plan.currentMiddle).reverse();
		if (deletionChunks.length > STREAM_EDIT_MAX_CHUNKS) {
			bridge.recordDebug(`${source}: too many deletion chunks -> write_file ${formatDebugPath(absolutePath)}`);
			await bridge.writeFile(absolutePath, content);
			return;
		}
		let currentEndChar = plan.endChar;
		for (const [index, chunk] of deletionChunks.entries()) {
			const chunkLength = countCodePoints(chunk);
			bridge.recordDebug(
				`${source}: deletion chunk ${index + 1}/${deletionChunks.length} ${formatDebugPath(absolutePath)} [${currentEndChar - chunkLength},${currentEndChar}) len=${chunkLength}`,
			);
			await bridge.replaceRange(absolutePath, currentEndChar - chunkLength, currentEndChar, "");
			currentEndChar -= chunkLength;
			await sleep(STREAM_EDIT_DELAY_MS);
		}
		return;
	}

	const insertionChunks = tokenizeStreamingText(plan.targetMiddle);
	if (insertionChunks.length > STREAM_EDIT_MAX_CHUNKS) {
		bridge.recordDebug(`${source}: too many insertion chunks -> write_file ${formatDebugPath(absolutePath)}`);
		await bridge.writeFile(absolutePath, content);
		return;
	}
	const [firstChunk, ...remainingChunks] = insertionChunks;
	if (!firstChunk) {
		bridge.recordDebug(`${source}: empty first chunk -> write_file ${formatDebugPath(absolutePath)}`);
		await bridge.writeFile(absolutePath, content);
		return;
	}
	bridge.recordDebug(
		`${source}: insertion chunk 1/${insertionChunks.length} ${formatDebugPath(absolutePath)} [${plan.startChar},${plan.endChar}) len=${countCodePoints(firstChunk)}`,
	);
	await bridge.replaceRange(absolutePath, plan.startChar, plan.endChar, firstChunk);
	let insertedChars = countCodePoints(firstChunk);
	await sleep(STREAM_EDIT_DELAY_MS);

	for (const [index, chunk] of remainingChunks.entries()) {
		bridge.recordDebug(
			`${source}: insertion chunk ${index + 2}/${insertionChunks.length} ${formatDebugPath(absolutePath)} [${plan.startChar + insertedChars},${plan.startChar + insertedChars}) len=${countCodePoints(chunk)}`,
		);
		await bridge.replaceRange(
			absolutePath,
			plan.startChar + insertedChars,
			plan.startChar + insertedChars,
			chunk,
		);
		insertedChars += countCodePoints(chunk);
		await sleep(STREAM_EDIT_DELAY_MS);
	}

	const finalContent = (await bridge.readFile(absolutePath, { sendPresence: false })).content;
	if (finalContent !== content) {
		bridge.recordDebug(
			`${source}: post-stream verification mismatch for ${formatDebugPath(absolutePath)} -> write_file`,
		);
		await bridge.writeFile(absolutePath, content);
	}
}

async function writeFileWithBridgeFallback(
	bridge: EditorBridgeClient,
	absolutePath: string,
	content: string,
	source: string,
): Promise<void> {
	await assertWorkspaceMutationBridgeReady(bridge, absolutePath);
	const manifest = await bridge.manifestForFilePath(absolutePath);
	if (manifest) {
		bridge.recordDebug(`${source}: write via bridge ${formatDebugPath(absolutePath)}`);
		try {
			await streamWriteFileWithBridge(bridge, absolutePath, content, source);
		} catch (error) {
			if (isBridgeTransportError(error) && (await bridge.requiresWorkspaceMutationBridge(absolutePath))) {
				throw new Error(editorBridgeRequiredMessage(absolutePath));
			}
			throw error;
		}
		return;
	}
	bridge.recordDebug(`${source}: write via fs fallback ${formatDebugPath(absolutePath)}`);
	await fsWriteFile(absolutePath, content, "utf8");
}

class EditorBridgeClient {
	private manifest: PiBridgeManifest | null = null;
	private socket: net.Socket | null = null;
	private buffer = "";
	private nextRequestId = 1;
	private state: BridgeState = "detached";
	private pending = new Map<
		string,
		{ resolve: (value: JsonValue | undefined) => void; reject: (error: Error) => void }
	>();
	private currentContext: any | null = null;
	private currentPi: ExtensionAPI | null = null;
	private hadAttachedStatus = false;
	private hasShownBusyWarning = false;
	private attachPromise: Promise<boolean> | null = null;
	private debugHistory: BridgeDebugEntry[] = [];
	private lastSubscriberDisconnectReason: string | null = null;
	private lastRpcFailure: string | null = null;
	private lastRpcSuccessAt: string | null = null;

	recordDebug(message: string): void {
		this.debugHistory.push({ at: new Date().toISOString(), message });
		if (this.debugHistory.length > 80) {
			this.debugHistory.shift();
		}
		if (PI_BRIDGE_DEBUG) {
			console.error(`[the-editor-pi-extension] ${message}`);
		}
	}

	getDebugHistory(): BridgeDebugEntry[] {
		return [...this.debugHistory];
	}

	async attach(cwd: string, ctx: any, pi: ExtensionAPI): Promise<boolean> {
		this.currentContext = ctx;
		this.currentPi = pi;
		if (this.attachPromise) {
			return this.attachPromise;
		}
		const promise = this.attachInternal(cwd);
		this.attachPromise = promise.finally(() => {
			if (this.attachPromise === promise) {
				this.attachPromise = null;
			}
		});
		return this.attachPromise;
	}

	private async attachInternal(cwd: string): Promise<boolean> {
		this.recordDebug(`events: attach start cwd=${cwd}`);
		const manifest = await findBridgeManifest(cwd);
		if (!manifest) {
			this.manifest = null;
			this.recordDebug(`events: attach skipped no manifest cwd=${cwd}`);
			this.disconnect("not found");
			this.setDetachedStatus("editor");
			return false;
		}
		if (
			this.isAttached() &&
			this.manifest?.socketPath === manifest.socketPath &&
			this.manifest.workspaceRoot === manifest.workspaceRoot
		) {
			this.recordDebug(`events: attach reuse ${manifest.workspaceRoot}`);
			this.setAttachedStatus(path.basename(manifest.workspaceRoot));
			return true;
		}

		this.closeSocket();
		this.rejectPending(new Error("editor bridge reconnecting"));
		this.state = "detached";
		this.manifest = manifest;
		this.lastSubscriberDisconnectReason = null;

		await new Promise<void>((resolve, reject) => {
			this.recordDebug(`events: subscriber connect ${manifest.socketPath}`);
			const socket = net.createConnection(manifest.socketPath, () => {
				this.socket = socket;
				this.recordDebug(`events: subscriber connected ${manifest.workspaceRoot}`);
				resolve();
			});

			socket.setEncoding("utf8");
			socket.on("data", (chunk: string) => {
				this.buffer += chunk;
				this.flushBufferedLines();
			});
			socket.on("error", (error) => {
				if (!this.socket || this.socket === socket) {
					this.recordDebug(`events: subscriber error ${describeError(error)}`);
					this.disconnect(error.message);
				}
			});
			socket.on("close", () => {
				if (!this.socket || this.socket === socket) {
					this.recordDebug("events: subscriber closed");
					this.disconnect("closed");
				}
			});
			socket.once("error", reject);
		});

		try {
			this.recordDebug("events: subscribe_events start");
			await this.sendNotification("subscribe_events", {});
			this.recordDebug("events: subscribe_events ok");
			this.recordDebug("rpc ping attach healthcheck start");
			await this.requestWithManifest(manifest, "ping", {});
			this.recordDebug("rpc ping attach healthcheck ok");
		} catch (error) {
			if (this.isBusy()) {
				return false;
			}
			const message = error instanceof Error ? error.message : "attach failed";
			this.recordDebug(`events: attach failed ${message}`);
			this.disconnect(message);
			throw error;
		}
		this.setAttachedStatus(path.basename(manifest.workspaceRoot));
		return true;
	}

	disconnect(reason?: string): void {
		if (reason) {
			this.lastSubscriberDisconnectReason = reason;
			this.recordDebug(`events: subscriber disconnect ${reason}`);
		}
		this.closeSocket();
		this.rejectPending(new Error("editor bridge disconnected"));
		this.attachPromise = null;
		this.state = "detached";
		this.hasShownBusyWarning = false;
		this.setDetachedStatus("editor");
	}

	isAttached(): boolean {
		return this.state === "attached" && !!this.socket && !this.socket.destroyed && !!this.manifest;
	}

	isBusy(): boolean {
		return this.state === "busy";
	}

	shouldAutoAttach(): boolean {
		return !this.isAttached() && !this.isBusy() && !this.attachPromise;
	}

	describeEvents(): string {
		if (this.attachPromise) {
			return `attaching to ${this.manifest?.workspaceRoot ?? "<unknown>"}`;
		}
		if (this.isBusy()) {
			return `busy: another pi session owns ${this.manifest?.workspaceRoot ?? "<unknown>"}`;
		}
		if (this.isAttached()) {
			return `attached to ${this.manifest?.workspaceRoot ?? "<unknown>"}`;
		}
		if (this.manifest) {
			return `detached from ${this.manifest.workspaceRoot}${this.lastSubscriberDisconnectReason ? ` (${this.lastSubscriberDisconnectReason})` : ""}`;
		}
		return this.lastSubscriberDisconnectReason ? `detached (${this.lastSubscriberDisconnectReason})` : "detached";
	}

	describeRpc(): string {
		if (!this.manifest) {
			return "unavailable";
		}
		if (this.lastRpcFailure) {
			return `degraded: ${this.lastRpcFailure}`;
		}
		if (this.lastRpcSuccessAt) {
			return `available (last ok ${this.lastRpcSuccessAt.split("T")[1]?.replace("Z", "") ?? this.lastRpcSuccessAt})`;
		}
		return "unknown";
	}

	workspaceRoot(): string | null {
		return this.manifest?.workspaceRoot ?? null;
	}

	workspaceRootHint(): string | null {
		return this.manifest?.workspaceRoot ?? this.currentContext?.cwd ?? null;
	}

	async manifestForFilePath(filePath: string): Promise<PiBridgeManifest | null> {
		if (!isProbablyTextPath(filePath)) {
			return null;
		}
		if (this.manifest && isPathInside(this.manifest.workspaceRoot, filePath)) {
			return this.manifest;
		}
		if (!this.currentContext) {
			return null;
		}
		const manifest = await findBridgeManifest(this.currentContext.cwd);
		if (!manifest || !isPathInside(manifest.workspaceRoot, filePath)) {
			return null;
		}
		return manifest;
	}

	async requiresWorkspaceMutationBridge(filePath: string): Promise<boolean> {
		return !!(await this.manifestForFilePath(filePath));
	}

	private markRpcSuccess(label: string): void {
		this.lastRpcFailure = null;
		this.lastRpcSuccessAt = new Date().toISOString();
		this.recordDebug(`rpc ${label} ok`);
	}

	private markRpcFailure(label: string, error: unknown): void {
		const detail = `${label}: ${describeError(error)}`;
		this.lastRpcFailure = detail;
		this.recordDebug(`rpc ${detail}`);
	}

	isWorkspaceTextPath(filePath: string): boolean {
		const workspaceRoot = this.workspaceRootHint();
		if (!workspaceRoot) {
			return false;
		}
		return isProbablyTextPath(filePath) && isPathInside(workspaceRoot, filePath);
	}

	shouldRoutePath(filePath: string): boolean {
		if (!this.isAttached() || !this.manifest) {
			return false;
		}
		return isProbablyTextPath(filePath) && isPathInside(this.manifest.workspaceRoot, filePath);
	}

	async tryAutoAttachForMutation(): Promise<void> {
		if (!this.currentContext || !this.currentPi || !this.shouldAutoAttach()) {
			return;
		}
		try {
			await this.attach(this.currentContext.cwd, this.currentContext, this.currentPi);
		} catch {
			// Keep mutation policy logic in the caller.
		}
	}

	async readFile(filePath: string, options: ReadFileOptions = {}): Promise<ReadFileResponse> {
		this.recordDebug(`rpc read_file ${formatDebugPath(filePath)}`);
		const manifest = await this.manifestForFilePath(filePath);
		if (!manifest) {
			throw new Error("editor bridge is not available for this path");
		}
		if (options.sendPresence !== false) {
			await this.trySendAgentPresence(manifest, "agent/focus", { path: filePath, kind: "read" });
		}
		const response = await this.requestWithManifest(manifest, "read_file", { path: filePath });
		return response as ReadFileResponse;
	}

	async writeFile(filePath: string, content: string): Promise<WriteFileResponse> {
		this.recordDebug(`rpc write_file ${formatDebugPath(filePath)} (${countCodePoints(content)} chars)`);
		const manifest = await this.manifestForFilePath(filePath);
		if (!manifest) {
			throw new Error("editor bridge is not available for this path");
		}
		await this.trySendAgentPresence(manifest, "agent/focus", {
			path: filePath,
			kind: "write",
			cursorChar: Math.max(countCodePoints(content) - 1, 0),
		});
		const response = await this.requestWithManifest(manifest, "write_file", { path: filePath, content });
		await this.trySendAgentPresence(manifest, "agent/edit_applied", {
			path: filePath,
			kind: "write",
			cursorChar: Math.max(countCodePoints(content) - 1, 0),
		});
		return response as WriteFileResponse;
	}

	async applyEdits(
		filePath: string,
		edits: Array<{ oldText: string; newText: string }>,
	): Promise<{ path: string; editCount: number; saved: boolean; openedBuffer: boolean }> {
		this.recordDebug(`rpc apply_edits ${formatDebugPath(filePath)} (${edits.length} edits)`);
		const manifest = await this.manifestForFilePath(filePath);
		if (!manifest) {
			throw new Error("editor bridge is not available for this path");
		}
		const baseline = await this.readFile(filePath, { sendPresence: false });
		const appliedRange = exactEditUnionRange(baseline.content, edits);
		await this.trySendAgentPresence(manifest, "agent/edit_preview", {
			path: filePath,
			kind: "edit_preview",
			range: { startChar: appliedRange.startChar, endChar: appliedRange.endChar },
			cursorChar: appliedRange.cursorChar,
		});
		const response = await this.requestWithManifest(manifest, "apply_edits", { path: filePath, edits });
		await this.trySendAgentPresence(manifest, "agent/edit_applied", {
			path: filePath,
			kind: "edit_applied",
			range: { startChar: appliedRange.startChar, endChar: appliedRange.endChar },
			cursorChar: appliedRange.cursorChar,
		});
		return response as { path: string; editCount: number; saved: boolean; openedBuffer: boolean };
	}

	async replaceRange(
		filePath: string,
		startChar: number,
		endChar: number,
		content: string,
	): Promise<ReplaceRangeResponse> {
		this.recordDebug(
			`rpc replace_range ${formatDebugPath(filePath)} [${startChar},${endChar}) -> ${countCodePoints(content)} chars`,
		);
		const manifest = await this.manifestForFilePath(filePath);
		if (!manifest) {
			throw new Error("editor bridge is not available for this path");
		}
		const finalEndChar = startChar + countCodePoints(content);
		await this.trySendAgentPresence(manifest, "agent/edit_preview", {
			path: filePath,
			kind: "edit_preview",
			range: { startChar, endChar },
			cursorChar: endChar,
		});
		const response = await this.requestWithManifest(manifest, "replace_range", {
			path: filePath,
			startChar,
			endChar,
			content,
		});
		await this.trySendAgentPresence(manifest, "agent/edit_applied", {
			path: filePath,
			kind: "edit_applied",
			range: { startChar, endChar: finalEndChar },
			cursorChar: finalEndChar,
		});
		return response as ReplaceRangeResponse;
	}

	private setAttachedStatus(label: string): void {
		this.state = "attached";
		if (!this.currentContext?.ui) {
			return;
		}
		this.currentContext.ui.setStatus(
			STATUS_KEY,
			formatStatus(this.currentContext.ui.theme, "attached", `editor:${label}`),
		);
		this.hadAttachedStatus = true;
	}

	private setDetachedStatus(label: string): void {
		this.state = "detached";
		if (!this.currentContext?.ui) {
			return;
		}
		if (!this.hadAttachedStatus) {
			this.currentContext.ui.setStatus(
				STATUS_KEY,
				formatStatus(this.currentContext.ui.theme, "detached", `${label}:off`),
			);
			return;
		}
		this.currentContext.ui.setStatus(
			STATUS_KEY,
			formatStatus(this.currentContext.ui.theme, "detached", `${label}:off`),
		);
		this.hadAttachedStatus = false;
	}

	private setBusyStatus(): void {
		this.state = "busy";
		if (!this.currentContext?.ui) {
			return;
		}
		this.currentContext.ui.setStatus(
			STATUS_KEY,
			formatStatus(this.currentContext.ui.theme, "busy", "editor:busy"),
		);
		this.hadAttachedStatus = true;
	}

	private closeSocket(): void {
		if (this.socket) {
			this.socket.removeAllListeners();
			this.socket.destroy();
		}
		this.socket = null;
		this.buffer = "";
	}

	private rejectPending(error: Error): void {
		for (const pending of this.pending.values()) {
			pending.reject(error);
		}
		this.pending.clear();
	}

	private transitionToBusy(reason: string): void {
		this.closeSocket();
		this.rejectPending(new Error(`editor bridge is ${reason}`));
		this.setBusyStatus();
		if (!this.hasShownBusyWarning && this.currentContext?.ui) {
			this.currentContext.ui.notify(
				"the-editor bridge is already attached to another pi session",
				"warning",
			);
			this.hasShownBusyWarning = true;
		}
	}

	refreshStatus(ctx: any): void {
		if (this.isAttached()) {
			this.setAttachedStatus(path.basename(this.workspaceRoot() || ctx.cwd));
			return;
		}
		if (this.isBusy()) {
			this.setBusyStatus();
			return;
		}
		this.setDetachedStatus("editor");
	}

	private flushBufferedLines(): void {
		let newlineIndex = this.buffer.indexOf("\n");
		let processed = 0;
		while (newlineIndex !== -1 && processed < MAX_BUFFERED_LINES) {
			const line = this.buffer.slice(0, newlineIndex).trim();
			this.buffer = this.buffer.slice(newlineIndex + 1);
			if (line.length > 0) {
				this.handleLine(line);
			}
			processed++;
			newlineIndex = this.buffer.indexOf("\n");
		}
	}

	private handleLine(line: string): void {
		let message: PiBridgeEnvelope;
		try {
			message = JSON.parse(line) as PiBridgeEnvelope;
		} catch {
			return;
		}

		if (message.type === "response") {
			const pending = this.pending.get(message.id);
			if (!pending) {
				return;
			}
			this.pending.delete(message.id);
			if (message.ok) {
				pending.resolve(message.result);
			} else {
				pending.reject(new Error(message.error || "editor bridge request failed"));
			}
			return;
		}

		if (message.type === "notification") {
			void this.handleNotification(message.method, message.params);
		}
	}

	private async handleNotification(method: string, params: JsonValue): Promise<void> {
		if (!this.currentContext || !this.currentPi) {
			return;
		}
		if (method === "attach_rejected") {
			const attachRejected = params as AttachRejectedParams;
			this.transitionToBusy(attachRejected.reason || "busy");
			return;
		}
		if (method !== "selection_prefill" && method !== "selection_send") {
			return;
		}

		const selection = params as SelectionPayload;
		const prompt = formatSelectionPrompt(selection);
		const workspaceLabel = path.basename(this.workspaceRoot() || this.currentContext.cwd);
		if (method === "selection_prefill") {
			const currentText = this.currentContext.ui.getEditorText();
			if (currentText.length === 0) {
				this.currentContext.ui.pasteToEditor(prompt);
			} else {
				this.currentContext.ui.setEditorText(prompt);
			}
			this.setAttachedStatus(workspaceLabel);
			return;
		}

		if (this.currentContext.isIdle()) {
			this.currentPi.sendUserMessage(prompt);
		} else {
			this.currentPi.sendUserMessage(prompt, { deliverAs: "steer" });
		}
		this.setAttachedStatus(workspaceLabel);
	}

	private async sendNotification(method: string, params: JsonValue): Promise<void> {
		const socket = this.socket;
		if (!socket || socket.destroyed) {
			throw new Error("editor bridge is not attached");
		}
		const envelope: PiBridgeEnvelope = {
			type: "notification",
			method,
			params,
		};
		return new Promise<void>((resolve, reject) => {
			socket.write(`${JSON.stringify(envelope)}
`, (error) => {
				if (error) {
					this.recordDebug(`events: notification ${method} write error ${describeError(error)}`);
					reject(error);
					return;
				}
				resolve();
			});
		});
	}

	private async sendNotificationWithManifest(
		manifest: PiBridgeManifest,
		method: string,
		params: JsonValue,
	): Promise<void> {
		this.recordDebug(`rpc ${method} notification socket connect ${manifest.socketPath}`);
		return new Promise<void>((resolve, reject) => {
			const socket = net.createConnection(manifest.socketPath, () => {
				this.recordDebug(`rpc ${method} notification socket connected`);
				this.sendNotificationOnSocket(socket, method, params).then(resolve, reject);
			});
			socket.setEncoding("utf8");
			socket.once("error", reject);
		});
	}

	private async sendNotificationOnSocket(
		socket: net.Socket,
		method: string,
		params: JsonValue,
	): Promise<void> {
		const envelope: PiBridgeEnvelope = {
			type: "notification",
			method,
			params,
		};
		return new Promise<void>((resolve, reject) => {
			let settled = false;
			const cleanup = () => {
				socket.removeListener("error", onError);
				socket.removeListener("close", onClose);
				socket.end();
			};
			const finish = (callback: () => void) => {
				if (settled) {
					return;
				}
				settled = true;
				cleanup();
				callback();
			};
			const onError = (error: Error) => finish(() => reject(error));
			const onClose = () => finish(() => resolve());
			socket.on("error", onError);
			socket.on("close", onClose);
			socket.write(`${JSON.stringify(envelope)}
`, (error) => {
				if (error) {
					finish(() => reject(error));
					return;
				}
				this.recordDebug(`rpc ${method} notification sent`);
				finish(() => resolve());
			});
		});
	}

	async trySendAgentPresence(
		manifest: PiBridgeManifest,
		method: string,
		params: AgentPresenceParams,
	): Promise<void> {
		try {
			await this.sendNotificationWithManifest(manifest, method, params);
		} catch (error) {
			this.recordDebug(`rpc ${method} notification skipped ${describeError(error)}`);
		}
	}

	private async request(method: string, params: JsonValue): Promise<JsonValue | undefined> {
		const socket = this.socket;
		if (!socket || socket.destroyed) {
			throw new Error("editor bridge is not attached");
		}
		const id = String(this.nextRequestId++);
		const envelope: PiBridgeEnvelope = {
			type: "request",
			id,
			method,
			params,
		};

		return new Promise<JsonValue | undefined>((resolve, reject) => {
			const timeout = setTimeout(() => {
				this.pending.delete(id);
				reject(new Error(`editor bridge request timed out (${method})`));
			}, BRIDGE_RPC_TIMEOUT_MS);
			this.pending.set(id, {
				resolve: (value) => {
					clearTimeout(timeout);
					resolve(value);
				},
				reject: (error) => {
					clearTimeout(timeout);
					reject(error);
				},
			});
			socket.write(`${JSON.stringify(envelope)}
`, (error) => {
				if (!error) {
					return;
				}
				clearTimeout(timeout);
				this.pending.delete(id);
				reject(error);
			});
		});
	}

	private async requestWithManifest(
		manifest: PiBridgeManifest,
		method: string,
		params: JsonValue,
	): Promise<JsonValue | undefined> {
		this.recordDebug(`rpc ${method} socket connect ${manifest.socketPath}`);
		return new Promise<JsonValue | undefined>((resolve, reject) => {
			const socket = net.createConnection(manifest.socketPath, () => {
				this.recordDebug(`rpc ${method} socket connected`);
				this.requestOnSocket(socket, method, params).then(resolve, reject);
			});
			socket.setEncoding("utf8");
			socket.once("error", (error) => {
				this.markRpcFailure(method, error);
				reject(error);
			});
		});
	}

	private async requestOnSocket(
		socket: net.Socket,
		method: string,
		params: JsonValue,
	): Promise<JsonValue | undefined> {
		const id = String(this.nextRequestId++);
		const envelope: PiBridgeEnvelope = {
			type: "request",
			id,
			method,
			params,
		};

		return new Promise<JsonValue | undefined>((resolve, reject) => {
			let settled = false;
			let buffer = "";
			const timeout = setTimeout(() => {
				finish(() => {
					const error = new Error(`editor bridge request timed out (${method})`);
					this.markRpcFailure(method, error);
					reject(error);
				});
			}, BRIDGE_RPC_TIMEOUT_MS);
			const cleanup = () => {
				clearTimeout(timeout);
				socket.removeListener("data", onData);
				socket.removeListener("error", onError);
				socket.removeListener("close", onClose);
				socket.end();
			};
			const finish = (callback: () => void) => {
				if (settled) {
					return;
				}
				settled = true;
				cleanup();
				callback();
			};
			const onError = (error: Error) => finish(() => {
				this.markRpcFailure(method, error);
				reject(error);
			});
			const onClose = () => finish(() => {
				const error = new Error("editor bridge disconnected");
				this.markRpcFailure(method, error);
				reject(error);
			});
			const onData = (chunk: string) => {
				buffer += chunk;
				let newlineIndex = buffer.indexOf("\n");
				while (newlineIndex !== -1) {
					const line = buffer.slice(0, newlineIndex).trim();
					buffer = buffer.slice(newlineIndex + 1);
					if (line.length > 0) {
						try {
							const message = JSON.parse(line) as PiBridgeEnvelope;
							if (message.type === "response" && message.id === id) {
								finish(() => {
									if (message.ok) {
										this.markRpcSuccess(method);
										resolve(message.result);
									} else {
										const error = new Error(message.error || "editor bridge request failed");
										this.markRpcFailure(method, error);
										reject(error);
									}
								});
								return;
							}
						} catch {
							// Ignore malformed line noise on one-off request sockets.
						}
					}
					newlineIndex = buffer.indexOf("\n");
				}
			};

			socket.on("data", onData);
			socket.on("error", onError);
			socket.on("close", onClose);
			socket.write(`${JSON.stringify(envelope)}
`, (error) => {
				if (!error) {
					this.recordDebug(`rpc ${method} request sent id=${id}`);
					return;
				}
				finish(() => {
					this.markRpcFailure(method, error);
					reject(error);
				});
			});
		});
	}

}

export default function (pi: ExtensionAPI) {
	const globalState = globalThis as typeof globalThis & {
		[EXTENSION_INSTANCE_KEY]?: ExtensionOwnerRecord;
	};
	if (globalState[EXTENSION_INSTANCE_KEY]) {
		return;
	}
	const ownerToken = {};
	globalState[EXTENSION_INSTANCE_KEY] = { token: ownerToken };

	const bridge = new EditorBridgeClient();
	const baseReadTool = createReadToolDefinition(process.cwd(), {
		operations: {
			access: async (absolutePath) => {
				await fsAccess(absolutePath, constants.R_OK);
			},
			readFile: async (absolutePath) => {
				return readFileWithBridgeFallback(bridge, absolutePath, "read");
			},
			detectImageMimeType: async (absolutePath) => {
				if (await bridge.manifestForFilePath(absolutePath)) {
					return null;
				}
				return detectImageMimeTypeFromPath(absolutePath);
			},
		},
	});
	const readTool = {
		...baseReadTool,
		execute: async (toolCallId, input, signal, onUpdate, ctx) => {
			const params = input as { path: string; offset?: number; limit?: number };
			const absolutePath = resolveToolPath(ctx.cwd, params.path);
			const manifest = await bridge.manifestForFilePath(absolutePath);
			const presence = readPresenceForInput(input);
			if (manifest && presence) {
				await bridge.trySendAgentPresence(manifest, "agent/focus", {
					path: absolutePath,
					kind: "read",
					startLine: presence.startLine,
					endLine: presence.endLine,
					cursorChar: lineNumberToCharIndex(await fsReadFile(absolutePath, "utf8").catch(() => ""), presence.startLine),
				});
			}
			return baseReadTool.execute(toolCallId, input, signal, onUpdate, ctx);
		},
	};
	const baseEditTool = createEditToolDefinition(process.cwd(), {
		operations: {
			access: async (absolutePath) => {
				await fsAccess(absolutePath, constants.R_OK | constants.W_OK);
			},
			readFile: async (absolutePath) => {
				return readFileWithBridgeFallback(bridge, absolutePath, "edit");
			},
			writeFile: async (absolutePath, content) => {
				await writeFileWithBridgeFallback(bridge, absolutePath, content, "edit");
			},
		},
	});
	const editTool = {
		...baseEditTool,
		execute: async (toolCallId, input, signal, onUpdate, ctx) => {
			const params = input as { path: string; edits: Array<{ oldText: string; newText: string }> };
			if (!Array.isArray(params.edits) || params.edits.length === 0) {
				throw new Error("Edit tool input is invalid. edits must contain at least one replacement.");
			}
			const absolutePath = resolveToolPath(ctx.cwd, params.path);
			return withFileMutationQueue(absolutePath, async () => {
				if (signal?.aborted) {
					throw new Error("Operation aborted");
				}
				await fsAccess(absolutePath, constants.R_OK | constants.W_OK);
				await assertWorkspaceMutationBridgeReady(bridge, absolutePath);
				if (await bridge.manifestForFilePath(absolutePath)) {
					bridge.recordDebug(`edit: apply_edits via bridge ${formatDebugPath(absolutePath)}`);
					try {
						await bridge.applyEdits(absolutePath, params.edits);
					} catch (error) {
						if (isBridgeTransportError(error) && (await bridge.requiresWorkspaceMutationBridge(absolutePath))) {
							throw new Error(editorBridgeRequiredMessage(absolutePath));
						}
						throw error;
					}
					return {
						content: [{ type: "text", text: `Successfully replaced ${params.edits.length} block(s) in ${params.path}.` }],
						details: undefined,
					};
				}
				const currentContent = await fsReadFile(absolutePath, "utf8");
				const nextContent = applyExactEditsLocally(currentContent, params.edits, params.path);
				bridge.recordDebug(`edit: local fs write ${formatDebugPath(absolutePath)}`);
				await fsWriteFile(absolutePath, nextContent, "utf8");
				return {
					content: [{ type: "text", text: `Successfully replaced ${params.edits.length} block(s) in ${params.path}.` }],
					details: undefined,
				};
			});
		},
	};
	const writeTool = createWriteToolDefinition(process.cwd(), {
		operations: {
			mkdir: async (dir) => {
				await fsMkdir(dir, { recursive: true });
			},
			writeFile: async (absolutePath, content) => {
				await writeFileWithBridgeFallback(bridge, absolutePath, content, "write");
			},
		},
	});

	pi.registerTool({
		...readTool,
		label: "read (editor)",
	});
	pi.registerTool({
		...editTool,
		label: "edit (editor)",
	});
	pi.registerTool({
		...writeTool,
		label: "write (editor)",
	});

	pi.on("session_start", async (_event, ctx) => {
		try {
			await bridge.attach(ctx.cwd, ctx, pi);
		} catch (error) {
			ctx.ui.setStatus(STATUS_KEY, formatStatus(ctx.ui.theme, "detached", "editor:off"));
			ctx.ui.notify(`the-editor bridge attach failed: ${error instanceof Error ? error.message : String(error)}`, "warning");
		}
	});

	pi.on("session_shutdown", async () => {
		const workspaceRoot = bridge.workspaceRoot();
		if (workspaceRoot) {
			const manifest = await findBridgeManifest(workspaceRoot).catch(() => null);
			if (manifest) {
				await bridge.trySendAgentPresence(manifest, "agent/end", { path: workspaceRoot });
			}
		}
		bridge.disconnect("session shutdown");
		if (globalState[EXTENSION_INSTANCE_KEY]?.token === ownerToken) {
			delete globalState[EXTENSION_INSTANCE_KEY];
		}
	});

	pi.registerCommand("the-editor-status", {
		description: "Show current the-editor bridge status",
		handler: async (_args, ctx) => {
			const root = bridge.workspaceRoot();
			const recent = bridge
				.getDebugHistory()
				.slice(-12)
				.map((entry) => `${entry.at.split("T")[1]?.replace("Z", "") ?? entry.at} ${entry.message}`);
			const details = [
				`events: ${bridge.describeEvents()}`,
				`rpc: ${bridge.describeRpc()}`,
				`cwd: ${ctx.cwd}`,
				`workspace: ${root ?? "<none>"}`,
				"recent:",
				...(recent.length > 0 ? recent : ["<no debug events yet>"]),
			];
			ctx.ui.notify(details.join("\n"), "info");
		},
	});

	pi.registerCommand("the-editor-reconnect", {
		description: "Reconnect to the-editor bridge for the current workspace",
		handler: async (_args, ctx) => {
			try {
				const attached = await bridge.attach(ctx.cwd, ctx, pi);
				if (attached) {
					ctx.ui.notify("reconnected to the-editor", "info");
					return;
				}
				if (bridge.isBusy()) {
					ctx.ui.notify("the-editor bridge is busy; another pi session owns it", "warning");
					return;
				}
				ctx.ui.notify("the-editor bridge is not available for this workspace", "warning");
			} catch (error) {
				ctx.ui.notify(
					`failed to reconnect to the-editor: ${error instanceof Error ? error.message : String(error)}`,
					"error",
				);
			}
		},
	});

	pi.registerCommand("the-editor-send", {
		description: "Send a one-off prompt to the agent from the editor bridge status line",
		handler: async (args, ctx) => {
			const prompt = args.trim();
			if (!prompt) {
				ctx.ui.notify("usage: /the-editor-send <prompt>", "warning");
				return;
			}
			if (ctx.isIdle()) {
				pi.sendUserMessage(prompt);
			} else {
				pi.sendUserMessage(prompt, { deliverAs: "steer" });
			}
		},
	});

	pi.on("resources_discover", async (_event, ctx) => {
		bridge.refreshStatus(ctx);
	});

	pi.on("turn_end", async (_event, ctx) => {
		bridge.refreshStatus(ctx);
	});

	pi.on("turn_start", async (_event, ctx) => {
		if (!bridge.isAttached()) {
			return;
		}
		ctx.ui.setStatus(
			STATUS_KEY,
			formatStatus(ctx.ui.theme, "attached", `editor:${path.basename(bridge.workspaceRoot() || ctx.cwd)}`),
		);
	});

	pi.on("before_agent_start", async (_event, ctx) => {
		if (bridge.shouldAutoAttach()) {
			await bridge.attach(ctx.cwd, ctx, pi).catch(() => {});
		}
	});

	pi.on("input", async (_event, ctx) => {
		if (bridge.shouldAutoAttach()) {
			await bridge.attach(ctx.cwd, ctx, pi).catch(() => {});
		}
	});

	pi.on("tool_execution_start", async (event, _ctx) => {
		if (event.toolName === "edit" || event.toolName === "write") {
			const args = event.args as { path?: string } | undefined;
			bridge.recordDebug(`tool ${event.toolName} start ${args?.path ? formatDebugPath(args.path) : "<unknown>"}`);
		}
	});

	pi.on("tool_execution_end", async (event, ctx) => {
		if (event.toolName === "edit" || event.toolName === "write") {
			const resultText = Array.isArray(event.result?.content)
				? event.result.content
						.filter((item: { type?: string; text?: string }) => item.type === "text" && item.text)
						.map((item: { text?: string }) => item.text)
						.join(" ")
				: "";
			const summary = resultText.length > 140 ? `${resultText.slice(0, 140)}…` : resultText || "<no text>";
			bridge.recordDebug(`tool ${event.toolName} ${event.isError ? "error" : "ok"}: ${summary}`);
		}
		bridge.refreshStatus(ctx);
	});

	pi.on("agent_end", async (_event, ctx) => {
		bridge.refreshStatus(ctx);
	});

	pi.on("message_update", async (_event, ctx) => {
		if (bridge.isAttached()) {
			ctx.ui.setStatus(
				STATUS_KEY,
				formatStatus(ctx.ui.theme, "attached", `editor:${path.basename(bridge.workspaceRoot() || ctx.cwd)}`),
			);
		}
	});

	pi.on("context", async (_event, ctx) => {
		if (bridge.shouldAutoAttach()) {
			await bridge.attach(ctx.cwd, ctx, pi).catch(() => {});
		}
	});

	pi.on("message_end", async (_event, ctx) => {
		if (bridge.isAttached()) {
			ctx.ui.setStatus(
				STATUS_KEY,
				formatStatus(ctx.ui.theme, "attached", `editor:${path.basename(bridge.workspaceRoot() || ctx.cwd)}`),
			);
		}
	});

	pi.on("agent_start", async (_event, ctx) => {
		if (bridge.shouldAutoAttach()) {
			await bridge.attach(ctx.cwd, ctx, pi).catch(() => {});
		}
	});
}
