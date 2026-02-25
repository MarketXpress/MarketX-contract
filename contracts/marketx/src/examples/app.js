// Utility to log output
function log(msg) {
  document.getElementById("output").textContent += msg + "\n";
}

// Connect Freighter
async function connectWallet() {
  if (!window.freighterApi) {
    alert("Freighter wallet not found. Please install it.");
    return null;
  }
  const publicKey = await window.freighterApi.getPublicKey();
  log("Connected wallet: " + publicKey);
  return publicKey;
}

// Generic contract call
async function callContract(method, args) {
  const publicKey = await connectWallet();
  if (!publicKey) return;

  // Construct transaction (Soroban example)
  const tx = {
    method,
    args,
    sender: publicKey,
  };

  // Sign with Freighter
  const signedTx = await window.freighterApi.signTransaction(JSON.stringify(tx), {
    networkPassphrase: "Test SDF Network ; September 2015",
  });

  log("Signed transaction: " + signedTx);

  // Submit transaction to Horizon testnet
  const response = await fetch("https://horizon-testnet.stellar.org/transactions", {
    method: "POST",
    body: signedTx,
    headers: { "Content-Type": "application/json" },
  });

  const result = await response.json();
  log("Transaction result: " + JSON.stringify(result, null, 2));
}

// Bind buttons
document.getElementById("create").onclick = () =>
  callContract("create", { projectId: "p1", amount: "100" });

document.getElementById("fund").onclick = () =>
  callContract("fund", { projectId: "p1", amount: "50" });

document.getElementById("release").onclick = () =>
  callContract("release", { projectId: "p1" });
