const fs = require('fs');

async function checkAllNodes() {
    const baseUrl = 'https://spatial.planning.vic.gov.au/server/rest/services/Hosted/AQ_North/SceneServer/layers/0';
    for (let id = 0; id <= 18; id++) {
        try {
            const res = await fetch(`${baseUrl}/nodes/${id}/geometries/0`);
            if (!res.ok) continue;
            const buf = Buffer.from(await res.arrayBuffer());
            const vCount = buf.readUInt32LE(0);
            const fCount = buf.readUInt32LE(4);
            const colOffset = 8 + vCount * 12 + vCount * 12 + vCount * 8;
            const unique = new Set();
            for (let i = 0; i < vCount; i++) {
                const o = colOffset + i * 4;
                unique.add(`${buf[o]},${buf[o+1]},${buf[o+2]},${buf[o+3]}`);
            }
            console.log(`Node ${id}: vCount=${vCount} fCount=${fCount} colors=`, Array.from(unique));
        } catch (e) {
            console.log(`Node ${id}: error`, e.message);
        }
    }
}

checkAllNodes().catch(console.error);
