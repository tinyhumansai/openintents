"""uv run --with openintents sdk/examples/buy_coffee.py  (needs OPENINTENTS_API_KEY; use an oi_test_ key)"""

from openintents import OpenIntents

with OpenIntents() as oi:
    created = oi.intents.create("flat white, oat milk, pickup 8:45 at the Blue Bottle on 5th", max_amount=1000)
    print("created", created["id"])

    intent = oi.intents.wait(created["id"])
    if intent["status"] == "requires_payment":
        print("Pay here:", intent["payment"]["payment_url"])
    elif intent["status"] == "completed":
        print("Receipt:", intent["result"])
    else:
        print(f"Ended as {intent['status']}:", (intent.get("error") or {}).get("message"))
