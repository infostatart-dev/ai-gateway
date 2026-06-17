import http from 'k6/http';
import { check, sleep } from 'k6';

const GATEWAY_URL = __ENV.GATEWAY_URL || 'http://127.0.0.1:8080';
const MODEL =
  __ENV.AI_GATEWAY_AUTODEFAULT_DEFAULT_MODEL || 'openai/gpt-5.4-nano';

export const options = {
  vus: 2,
  duration: '10s',
  thresholds: {
    http_req_failed: ['rate<0.05'],
  },
};

export default function () {
  const payload = JSON.stringify({
    model: MODEL,
    messages: [{ role: 'user', content: 'hello world' }],
  });
  const res = http.post(
    `${GATEWAY_URL}/router/autodefault/chat/completions`,
    payload,
    { headers: { 'Content-Type': 'application/json' } },
  );
  check(res, {
    'status is 200': (r) => r.status === 200,
    'has usage': (r) => {
      try {
        return JSON.parse(r.body).usage?.prompt_tokens > 0;
      } catch {
        return false;
      }
    },
  });
  sleep(0.2);
}

export function handleSummary(data) {
  const stats = http.get(`${GATEWAY_URL}/v1/observability/provider-stats`);
  return {
    stdout: [
      JSON.stringify(data, null, 2),
      '\n--- provider-stats ---\n',
      stats.body,
    ].join('\n'),
  };
}
