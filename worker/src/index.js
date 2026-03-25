export default {
  async fetch(request, env) {
    const url = new URL(request.url);
    const headers = { "Content-Type": "application/json" };

    if (request.method === "GET" && url.pathname === "/total") {
      const res = await fetch(`${env.UPSTASH_URL}/GET/total_tokens_saved`, {
        headers: { Authorization: `Bearer ${env.UPSTASH_TOKEN}` },
      });
      const data = await res.json();
      return Response.json({ total: parseInt(data.result || "0", 10) }, { headers });
    }

    if (request.method === "POST" && url.pathname === "/increment") {
      const body = await request.json().catch(() => null);
      if (!body || !Number.isInteger(body.amount) || body.amount < 1 || body.amount > 10000000) {
        return Response.json({ error: "Invalid amount" }, { status: 400, headers });
      }
      const res = await fetch(
        `${env.UPSTASH_URL}/INCRBY/total_tokens_saved/${body.amount}`,
        { headers: { Authorization: `Bearer ${env.UPSTASH_TOKEN}` } }
      );
      const data = await res.json();
      return Response.json({ total: data.result }, { headers });
    }

    return Response.json({ error: "Not found" }, { status: 404, headers });
  },
};
