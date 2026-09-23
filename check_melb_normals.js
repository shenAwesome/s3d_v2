async function checkMelbourneNormals() {
    const baseUrl = 'https://spatial.planning.vic.gov.au/server/rest/services/Hosted/AB_Melbourne_WM/SceneServer/layers/0';
    const res = await fetch(`${baseUrl}/nodes/1/geometries/0`);
    if (!res.ok) { console.log('failed', res.status); return; }
    const buf = Buffer.from(await res.arrayBuffer());
    const vCount = buf.readUInt32LE(0);
    const normOffset = 8 + vCount * 12;
    console.log('Melbourne Node 1 vertexCount:', vCount);
    for (let i = 0; i < 5; i++) {
        const nx = buf.readFloatLE(normOffset + i * 12);
        const ny = buf.readFloatLE(normOffset + i * 12 + 4);
        const nz = buf.readFloatLE(normOffset + i * 12 + 8);
        console.log(`v${i}: n=(${nx.toFixed(4)}, ${ny.toFixed(4)}, ${nz.toFixed(4)}) len=${Math.hypot(nx, ny, nz).toFixed(4)}`);
    }
}

checkMelbourneNormals().catch(console.error);
