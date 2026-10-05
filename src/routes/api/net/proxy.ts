import { lookup } from "node:dns/promises";
import { isIP } from "node:net";
import { createFileRoute } from "@tanstack/react-router";

const BODY_LIMIT = 1.5 * 1024 * 1024;
const MAX_REDIRECTS = 4;
const REQUEST_TIMEOUT_MS = 12_000;

function isPrivateIpv4(input: string): boolean {
  const parts = input.split(".").map(Number);
  if (parts.length !== 4 || parts.some((part) => !Number.isInteger(part) || part < 0 || part > 255)) return true;
  const [a, b] = parts;
  return a === 0 || a === 10 || a === 127 || a >= 224 ||
    (a === 100 && b >= 64 && b <= 127) ||
    (a === 169 && b === 254) ||
    (a === 172 && b >= 16 && b <= 31) ||
    (a === 192 && (b === 0 || b === 168)) ||
    (a === 198 && (b === 18 || b === 19 || b === 51)) ||
    (a === 203 && b === 0);
}

function isPrivateIpv6(input: string): boolean {
  const address = input.toLowerCase().split("%", 1)[0] ?? "";
  if (address.startsWith("::ffff:")) {
    const mapped = address.slice(7);
    if (isIP(mapped) === 4) return isPrivateIpv4(mapped);
    const groups = mapped.split(":");
    if (groups.length === 2) {
      const value = Number.parseInt(groups[0] ?? "0", 16) * 65536 + Number.parseInt(groups[1] ?? "0", 16);
      return isPrivateIpv4(`${(value >>> 24) & 255}.${(value >>> 16) & 255}.${(value >>> 8) & 255}.${value & 255}`);
    }
    return true;
  }
  return address === "::" || address === "::1" || address.startsWith("fc") ||
    address.startsWith("fd") || address.startsWith("fe8") || address.startsWith("fe9") ||
    address.startsWith("fea") || address.startsWith("feb") || address.startsWith("ff") ||
    address.startsWith("2001:db8:");
}

async function publicUrl(raw: string): Promise<URL> {
  let url: URL;
  try {
    url = new URL(raw);
  } catch {
    throw new Error("Enter a valid web address.");
  }
  if (url.protocol !== "http:" && url.protocol !== "https:") throw new Error("Only HTTP and HTTPS pages are supported.");
  if (url.username || url.password) throw new Error("Addresses with embedded credentials are blocked.");
  if (url.port && url.port !== "80" && url.port !== "443") throw new Error("Only standard web ports 80 and 443 are supported.");
  const host = url.hostname.toLowerCase().replace(/^\[|\]$/g, "");
  if (!host || host === "localhost" || host.endsWith(".localhost") || host.endsWith(".local") || !host.includes(".")) {
    throw new Error("Private and local network addresses are blocked.");
  }
  const version = isIP(host);
  if (version === 4 && isPrivateIpv4(host)) throw new Error("Private network addresses are blocked.");
  if (version === 6 && isPrivateIpv6(host)) throw new Error("Private network addresses are blocked.");
  if (!version) {
    let addresses: Array<{ address: string; family: number }>;
    try {
      addresses = await lookup(host, { all: true, verbatim: true });
    } catch {
      throw new Error("That host could not be resolved.");
    }
    if (addresses.length === 0 || addresses.some(({ address, family }) => family === 4 ? isPrivateIpv4(address) : isPrivateIpv6(address))) {
      throw new Error("This address resolves to a private network and is blocked.");
    }
  }
  url.hash = "";
  return url;
}

async function readCapped(response: Response): Promise<Uint8Array> {
  const reader = response.body?.getReader();
  if (!reader) return new Uint8Array();
  const chunks: Uint8Array[] = [];
  let total = 0;
  try {
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      total += value.length;
      if (total > BODY_LIMIT) {
        await reader.cancel();
        throw new Error("This page is larger than the 1.5 MiB guest limit.");
      }
      chunks.push(value);
    }
  } finally {
    reader.releaseLock();
  }
  const body = new Uint8Array(total);
  let offset = 0;
  for (const chunk of chunks) {
    body.set(chunk, offset);
    offset += chunk.length;
  }
  return body;
}

async function proxyRequest(request: Request): Promise<Response> {
  try {
    const payload = await request.json() as { url?: unknown };
    if (typeof payload.url !== "string" || payload.url.length > 2048) throw new Error("A valid URL is required.");
    let url = await publicUrl(payload.url);
    let response: Response | undefined;
    for (let redirects = 0; redirects <= MAX_REDIRECTS; redirects += 1) {
      response = await fetch(url, { method: "GET", redirect: "manual", signal: AbortSignal.timeout(REQUEST_TIMEOUT_MS), headers: { "user-agent": "NEXUS-Virtual-Computer/0.1" } });
      if (![301, 302, 303, 307, 308].includes(response.status)) break;
      const location = response.headers.get("location");
      if (!location || redirects === MAX_REDIRECTS) throw new Error("The page redirected too many times.");
      url = await publicUrl(new URL(location, url).toString());
    }
    if (!response) throw new Error("No response from the website.");
    const body = await readCapped(response);
    const contentType = response.headers.get("content-type")?.split(";", 1)[0]?.slice(0, 100) ?? "text/html";
    const bodyBuffer = body.buffer.slice(body.byteOffset, body.byteOffset + body.byteLength) as ArrayBuffer;
    return new Response(bodyBuffer, { status: 200, headers: { "content-type": `${contentType}; charset=utf-8`, "cache-control": "no-store", "x-nexus-origin-status": String(response.status) } });
  } catch (error) {
    const message = error instanceof Error ? error.message : "Network request failed.";
    const status = message.includes("blocked") || message.includes("Only HTTP") || message.includes("standard web") ? 403 : 502;
    return new Response(message, { status, headers: { "content-type": "text/plain; charset=utf-8", "cache-control": "no-store" } });
  }
}

export const Route = createFileRoute("/api/net/proxy")({
  server: { handlers: { POST: ({ request }) => proxyRequest(request) } },
});
