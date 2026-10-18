import json,struct,sys
symf,prg,out=sys.argv[1:4]
j=json.load(open(symf)); dg=j['dgroup']
d=open(prg,'rb').read()
(sig,cblp,cp,crlc,cparhdr)=struct.unpack('<5H',d[:10]); imgsize=cp*512-(512-cblp if cblp else 0); img=d[cparhdr*16:imgsize]
ds=sorted([s for s in j['syms'] if s['seg']==dg and s['name']],key=lambda s:s['off'])
skip=('__','EMWS','_GR','BGI','FON_','DDO_','STAT','FPE','DGROUP')
fo=open(out,'w')
for i,s in enumerate(ds):
    if any(s['name'].startswith(k) or k in s['name'] for k in skip) and s['name']!='_STATMECH': continue
    o=s['off']; n=ds[i+1]['off'] if i+1<len(ds) else o+16; size=n-o
    b=img[dg*16+o:dg*16+n]
    if size>2600 or not any(b):
        fo.write(f"{s['name']} @{o:04x} size={size} {'(all zero)' if not any(b) else '(large, skipped)'}\n"); continue
    if all(32<=c<127 or c==0 for c in b) and size>=3 and b[0]!=0:
        fo.write(f"{s['name']} @{o:04x} size={size} STRINGS={[x for x in b.split(b'\\0') if x][:60]}\n"); continue
    fo.write(f"{s['name']} @{o:04x} size={size} bytes={list(b)}\n")
    if size%2==0: fo.write(f"    as_shorts={list(struct.unpack('<%dh'%(size//2),b))}\n")
    if size%4==0 and size<=64: fo.write(f"    as_longs={list(struct.unpack('<%dl'%(size//4),b))} as_floats={[round(x,4) for x in struct.unpack('<%df'%(size//4),b)]}\n")
fo.close()
