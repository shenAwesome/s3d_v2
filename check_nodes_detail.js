const fs = require('fs');

async function checkNodesDetail() {
    const baseUrl = 'https://spatial.planning.vic.gov.au/server/rest/services/Hosted/AQ_North/SceneServer/layers/0';
    for (let nodeId = 1; nodeId <= 18; nodeId++) {
        try {
            const geomRes = await fetch(`${baseUrl}/nodes/${nodeId}/geometries/0`);
            if (!geomRes.ok) continue;
            const geomBuf = Buffer.from(await geomRes.arrayBuffer());
            if (geomBuf.length < 8) continue;
            const vertexCount = geomBuf.readUInt32LE(0);
            const featureCount = geomBuf.readUInt32LE(4);
            const posOffset = 8;
            const normOffset = posOffset + vertexCount * 12;
            const uvOffset = normOffset + vertexCount * 12;
            const colorOffset = uvOffset + vertexCount * 8;

            const counts = {};
            for (let i = 0; i < vertexCount; i++) {
                const o = colorOffset + i * 4;
                const col = `${geomBuf[o]},${geomBuf[o+1]},${geomBuf[o+2]},${geomBuf[o+3]}`;
                counts[col] = (counts[col] || 0) + 1;
            }
            console.log(`Node ${nodeId} (vCount: ${vertexCount}):`, counts);
        } catch (e) {
            console.error(nodeId, e.message);
        }
    }
}

checkNodesDetail().catch(console.error);
