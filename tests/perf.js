import { check } from "k6";
import http from "k6/http";

export const options = {
  scenarios: {
    load: {
      executor: "constant-arrival-rate",
      rate: Number(__ENV.RATE),
      timeUnit: "1s",
      duration: "60s",
      preAllocatedVUs: 50,
      maxVUs: 400,
    },
  },
  summaryTrendStats: ["p(50)", "p(95)", "p(99)"],
};

const headers = { Host: `${__ENV.TENANT}.blastradius.localhost` };

export default function () {
  const res = http.get("http://localhost:8084/api/quote?items=100", { headers, timeout: "5s" });
  check(res, { ok: (r) => r.status === 200 });
}

export function handleSummary(data) {
  return { [__ENV.SUMMARY]: JSON.stringify(data.metrics) };
}
