const fs = require('fs');

async function checkAllNodes() {
    const baseUrl = 'https://spatial.planning.vic.gov.au/server/rest/services/Hosted/AQ_North/SceneServer/layers/0';
    const allUnique = new Set();
    for (let nodeId = 1; nodeId <= 19; nodeId++) {
        try {
            const geomRes = await fetch(`${baseUrl}/nodes/${nodeId}/geometries/0`);
            if (!geomRes.ok) continue;
            const geomBuf = Buffer.from(await geomRes.arrayBuffer());
            const vertexCount = geomBuf.readUInt32LE(0);
            const posOffset = 8;
            const normOffset = posOffset + vertexCount * 12;
            const uvOffset = normOffset + vertexCount * 12;
            const colorOffset = uvOffset + vertexCount * 8;
            for (let i = 0; i < vertexCount; i++) {
                const o = colorOffset + i * 4;
                allUnique.add(`${geomBuf[o]},${geomBuf[o+1]},${geomBuf[o+2]},${geomBuf[o+3]}`);
            }
        } catch (e) {
            console.error(nodeId, e.message);
        }
    }
    console.log('Unique colors across all nodes:', Array.from(allUnique));
}

checkAllNodes().catch(console.error);
