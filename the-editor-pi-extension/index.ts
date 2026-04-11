import { createEditToolDefinition, createReadToolDefinition, createWriteToolDefinition, type ExtensionAPI, withFileMutationQueue } from "@mariozechner/pi-coding-agent";
import { access as fsAccess, mkdir as fsMkdir, readFile as fsReadFile, stat as fsStat, writeFile as fsWriteFile } from "node:fs/promises";
import net from "node:net";
import path from "node:path";
import { constants } from "node:fs";

const STATUS_KEY = "the-editor";
const GIT_MANIFEST_RELATIVE_PATH = path.join("the-editor", "pi-bridge.json");
const MAX_BUFFERED_LINES = 1000;
const EXTENSION_INSTANCE_KEY = "__the_editor_pi_extension_loaded__";
const STREAM_EDIT_DELAY_MS = 2;
const STREAM_EDIT_MAX_CHANGED_CODEPOINTS = 1800;
const STREAM_EDIT_MAX_CHUNKS = 24;

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

function streamChunkSizeFor(text: string): number {
	const length = countCodePoints(text);
	if (length <= 64) {
		return 16;
	}
	if (length <= 256) {
		return 32;
	}
	return 64;
}

function splitTokenForStreaming(token: string, chunkSize: number): string[] {
	const chars = Array.from(token);
	const chunks: string[] = [];
	for (let index = 0; index < chars.length; index += chunkSize) {
		chunks.push(chars.slice(index, index + chunkSize).join(""));
	}
	return chunks;
}

function tokenizeStreamingText(text: string): string[] {
	if (text.length === 0) {
		return [];
	}
	const chunkSize = streamChunkSizeFor(text);
	const rawTokens = text.match(/\r\n|\n|[ \t]+|[A-Za-z0-9_]+|./gu) ?? [];
	const chunks: string[] = [];
	for (const token of rawTokens) {
		if (token === "\n" || token === "\r\n") {
			chunks.push(token);
			continue;
		}
		chunks.push(...splitTokenForStreaming(token, chunkSize));
	}
	return chunks.filter((chunk) => chunk.length > 0);
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
		message.includes("epipe")
	);
}

async function withBridgeFallback<T>(
	bridge: EditorBridgeClient,
	filePath: string,
	bridgeOperation: () => Promise<T>,
	fallbackOperation: () => Promise<T>,
): Promise<T> {
	if (!bridge.shouldRoutePath(filePath)) {
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

function assertWorkspaceMutationBridgeReady(bridge: EditorBridgeClient, filePath: string): void {
	if (bridge.isWorkspaceTextPath(filePath) && !bridge.isAttached()) {
		throw new Error(editorBridgeRequiredMessage(filePath));
	}
}

async function readFileWithBridgeFallback(
	bridge: EditorBridgeClient,
	absolutePath: string,
	source: string,
): Promise<Buffer> {
	return withBridgeFallback(
		bridge,
		absolutePath,
		async () => {
			bridge.recordDebug(`${source}: read via bridge ${formatDebugPath(absolutePath)}`);
			const response = await bridge.readFile(absolutePath);
			return Buffer.from(response.content, "utf8");
		},
		async () => {
			bridge.recordDebug(`${source}: read via fs fallback ${formatDebugPath(absolutePath)}`);
			return fsReadFile(absolutePath);
		},
	);
}

async function readTextFileForStreaming(bridge: EditorBridgeClient, absolutePath: string, source: string): Promise<string> {
	try {
		if (bridge.shouldRoutePath(absolutePath)) {
			bridge.recordDebug(`${source}: stream baseline via bridge ${formatDebugPath(absolutePath)}`);
			return (await bridge.readFile(absolutePath)).content;
		}
		bridge.recordDebug(`${source}: stream baseline via fs ${formatDebugPath(absolutePath)}`);
		return await fsReadFile(absolutePath, "utf8");
	} catch (error) {
		if (isMissingFileError(error)) {
			return "";
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
	const currentContent = await readTextFileForStreaming(bridge, absolutePath, source);
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
		`${source}: streaming ${formatDebugPath(absolutePath)} (${changedCodePoints} codepoints changed)`,
	);

	if (plan.targetMiddle.length === 0) {
		const deletionChunks = tokenizeStreamingText(plan.currentMiddle).reverse();
		if (deletionChunks.length > STREAM_EDIT_MAX_CHUNKS) {
			bridge.recordDebug(`${source}: too many deletion chunks -> write_file ${formatDebugPath(absolutePath)}`);
			await bridge.writeFile(absolutePath, content);
			return;
		}
		let currentEndChar = plan.endChar;
		for (const chunk of deletionChunks) {
			const chunkLength = countCodePoints(chunk);
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
	await bridge.replaceRange(absolutePath, plan.startChar, plan.endChar, firstChunk);
	let insertedChars = countCodePoints(firstChunk);
	await sleep(STREAM_EDIT_DELAY_MS);

	for (const chunk of remainingChunks) {
		await bridge.replaceRange(
			absolutePath,
			plan.startChar + insertedChars,
			plan.startChar + insertedChars,
			chunk,
		);
		insertedChars += countCodePoints(chunk);
		await sleep(STREAM_EDIT_DELAY_MS);
	}
}

async function writeFileWithBridgeFallback(
	bridge: EditorBridgeClient,
	absolutePath: string,
	content: string,
	source: string,
): Promise<void> {
	assertWorkspaceMutationBridgeReady(bridge, absolutePath);
	if (bridge.shouldRoutePath(absolutePath)) {
		bridge.recordDebug(`${source}: write via bridge ${formatDebugPath(absolutePath)}`);
		try {
			await streamWriteFileWithBridge(bridge, absolutePath, content, source);
		} catch (error) {
			if (isBridgeTransportError(error) && bridge.isWorkspaceTextPath(absolutePath)) {
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

	recordDebug(message: string): void {
		this.debugHistory.push({ at: new Date().toISOString(), message });
		if (this.debugHistory.length > 40) {
			this.debugHistory.shift();
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
		const manifest = await findBridgeManifest(cwd);
		if (!manifest) {
			this.disconnect("not found");
			this.setDetachedStatus("editor");
			return false;
		}
		if (
			this.isAttached() &&
			this.manifest?.socketPath === manifest.socketPath &&
			this.manifest.workspaceRoot === manifest.workspaceRoot
		) {
			this.setAttachedStatus(path.basename(manifest.workspaceRoot));
			return true;
		}

		this.closeSocket();
		this.rejectPending(new Error("editor bridge reconnecting"));
		this.state = "detached";
		this.manifest = manifest;

		await new Promise<void>((resolve, reject) => {
			const socket = net.createConnection(manifest.socketPath, () => {
				this.socket = socket;
				resolve();
			});

			socket.setEncoding("utf8");
			socket.on("data", (chunk: string) => {
				this.buffer += chunk;
				this.flushBufferedLines();
			});
			socket.on("error", (error) => {
				if (!this.socket || this.socket === socket) {
					this.disconnect(error.message);
				}
			});
			socket.on("close", () => {
				if (!this.socket || this.socket === socket) {
					this.disconnect("closed");
				}
			});
			socket.once("error", reject);
		});

		try {
			await this.request("ping", {});
		} catch (error) {
			if (this.isBusy()) {
				return false;
			}
			this.disconnect(error instanceof Error ? error.message : "attach failed");
			throw error;
		}
		this.setAttachedStatus(path.basename(manifest.workspaceRoot));
		return true;
	}

	disconnect(_reason?: string): void {
		this.closeSocket();
		this.rejectPending(new Error("editor bridge disconnected"));
		this.attachPromise = null;
		this.manifest = null;
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

	describe(): string {
		if (this.attachPromise) {
			return `attaching to ${this.manifest?.workspaceRoot ?? "<unknown>"}`;
		}
		if (this.isBusy()) {
			return `busy: another pi session owns ${this.manifest?.workspaceRoot ?? "<unknown>"}`;
		}
		if (!this.manifest) {
			return "detached";
		}
		return `attached to ${this.manifest.workspaceRoot}`;
	}

	workspaceRoot(): string | null {
		return this.manifest?.workspaceRoot ?? null;
	}

	workspaceRootHint(): string | null {
		return this.manifest?.workspaceRoot ?? this.currentContext?.cwd ?? null;
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
		return this.isWorkspaceTextPath(filePath);
	}

	async readFile(filePath: string): Promise<ReadFileResponse> {
		this.recordDebug(`rpc read_file ${formatDebugPath(filePath)}`);
		const response = await this.request("read_file", { path: filePath });
		return response as ReadFileResponse;
	}

	async writeFile(filePath: string, content: string): Promise<WriteFileResponse> {
		this.recordDebug(`rpc write_file ${formatDebugPath(filePath)} (${countCodePoints(content)} chars)`);
		const response = await this.request("write_file", { path: filePath, content });
		return response as WriteFileResponse;
	}

	async applyEdits(
		filePath: string,
		edits: Array<{ oldText: string; newText: string }>,
	): Promise<{ path: string; editCount: number; saved: boolean; openedBuffer: boolean }> {
		this.recordDebug(`rpc apply_edits ${formatDebugPath(filePath)} (${edits.length} edits)`);
		const response = await this.request("apply_edits", { path: filePath, edits });
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
		const response = await this.request("replace_range", {
			path: filePath,
			startChar,
			endChar,
			content,
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
			this.pending.set(id, { resolve, reject });
			socket.write(`${JSON.stringify(envelope)}\n`, (error) => {
				if (!error) {
					return;
				}
				this.pending.delete(id);
				reject(error);
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
	const readTool = createReadToolDefinition(process.cwd(), {
		operations: {
			access: async (absolutePath) => {
				await fsAccess(absolutePath, constants.R_OK);
			},
			readFile: async (absolutePath) => {
				return readFileWithBridgeFallback(bridge, absolutePath, "read");
			},
			detectImageMimeType: async (absolutePath) => {
				if (bridge.shouldRoutePath(absolutePath)) {
					return null;
				}
				return detectImageMimeTypeFromPath(absolutePath);
			},
		},
	});
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
				assertWorkspaceMutationBridgeReady(bridge, absolutePath);
				if (bridge.shouldRoutePath(absolutePath)) {
					bridge.recordDebug(`edit: apply_edits via bridge ${formatDebugPath(absolutePath)}`);
					try {
						await bridge.applyEdits(absolutePath, params.edits);
					} catch (error) {
						if (isBridgeTransportError(error) && bridge.isWorkspaceTextPath(absolutePath)) {
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
				`bridge: ${bridge.describe()}`,
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
