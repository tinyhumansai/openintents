// npx tsx sdk/examples/buy-coffee.ts   (needs OPENINTENTS_API_KEY; use an oi_test_ key)
import { OpenIntents } from "@tinyhumansai/openintents";

const oi = new OpenIntents();

const created = await oi.intents.create({
  intent: "flat white, oat milk, pickup 8:45 at the Blue Bottle on 5th",
  max_amount: 1000, // cents
});
console.log("created", created.id);

const intent = await oi.intents.wait(created.id);
if (intent.status === "requires_payment") {
  console.log("Pay here:", intent.payment?.payment_url);
} else if (intent.status === "completed") {
  console.log("Receipt:", intent.result);
} else {
  console.log(`Ended as ${intent.status}:`, intent.error?.message);
}
