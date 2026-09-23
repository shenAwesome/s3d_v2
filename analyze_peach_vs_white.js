const fs = require('fs');

async function analyze() {
    const baseUrl = 'https://spatial.planning.vic.gov.au/server/rest/services/Hosted/AQ_North/SceneServer/layers/0';
    const res = await fetch(`${baseUrl}/nodes/1/geometries/0`);
    const buf = Buffer.from(await res.arrayBuffer());
    const vCount = buf.readUInt32LE(0);
    const posOffset = 8;
    const normOffset = posOffset + vCount * 12;
    const colOffset = normOffset + vCount * 12 + vCount * 8;

    const peachTriangles = [];
    const whiteTriangles = [];

    for (let t = 0; t < vCount / 3; t++) {
        const i = t * 3;
        const r = buf[colOffset + i * 4];
        const g = buf[colOffset + i * 4 + 1];
        const b = buf[colOffset + i * 4 + 2];

        const pts = [0, 1, 2].map(k => [
            buf.readFloatLE(posOffset + (i+k) * 12),
            buf.readFloatLE(posOffset + (i+k) * 12 + 4),
            buf.readFloatLE(posOffset + (i+k) * 12 + 8),
        ]);
        const norm = [
            buf.readFloatLE(normOffset + i * 12),
            buf.readFloatLE(normOffset + i * 12 + 4),
            buf.readFloatLE(normOffset + i * 12 + 8),
        ];

        if (r === 255 && g === 226) {
            peachTriangles.push({ t, pts, norm });
        } else if (r === 255 && g === 255) {
            whiteTriangles.push({ t, pts, norm });
        }
    }

    console.log(`Peach triangles: ${peachTriangles.length}, White triangles: ${whiteTriangles.length}`);

    // Check if any white triangle shares vertices with a peach triangle
    let exactMatches = 0;
    for (const p of peachTriangles) {
        for (const w of whiteTriangles) {
            // Check if pts match (either same order or reversed)
            const pCentroid = [
                (p.pts[0][0] + p.pts[1][0] + p.pts[2][0]) / 3,
                (p.pts[0][1] + p.pts[1][1] + p.pts[2][1]) / 3,
                (p.pts[0][2] + p.pts[1][2] + p.pts[2][2]) / 3,
            ];
            const wCentroid = [
                (w.pts[0][0] + w.pts[1][0] + w.pts[2][0]) / 3,
                (w.pts[0][1] + w.pts[1][1] + w.pts[2][1]) / 3,
                (w.pts[0][2] + w.pts[1][2] + w.pts[2][2]) / 3,
            ];
            const dist = Math.hypot(pCentroid[0]-wCentroid[0], pCentroid[1]-wCentroid[1], pCentroid[2]-wCentroid[2]);
            if (dist < 1e-4) {
                exactMatches++;
                console.log(`Match: Peach T${p.t} vs White T${w.t}, normal dot: ${p.norm[0]*w.norm[0]+p.norm[1]*w.norm[1]+p.norm[2]*w.norm[2]}`);
            }
        }
    }
    console.log(`Total exact geometric duplicate triangles: ${exactMatches}`);
}

analyze().catch(console.error);
