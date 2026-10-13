// Typed IPC bindings for Research Studio. Phase G.

import { invoke } from "@tauri-apps/api/core";

export type ResearchProject = {
  id: string;
  workspace_id: string | null;
  title: string;
  question: string;
  description: string | null;
  status: string;
  created_at: number;
  updated_at: number;
};

export type ResearchPaper = {
  id: string;
  research_project_id: string;
  document_id: string | null;
  title: string;
  authors: string | null;
  venue: string | null;
  year: number | null;
  doi: string | null;
  url: string | null;
  notes: string | null;
  position: number;
  created_at: number;
};

export type PaperClaim = {
  id: string;
  research_project_id: string;
  paper_id: string | null;
  document_id: string | null;
  claim: string;
  category: string;
  evidence: string | null;
  confidence: number;
  kg_entity_id: string | null;
  created_at: number;
};

export type ComparisonMatrix = {
  id: string;
  research_project_id: string;
  title: string;
  description: string | null;
  created_at: number;
};

export type MatrixColumn = {
  id: string;
  matrix_id: string;
  label: string;
  kind: string;
  position: number;
};

export type MatrixCell = {
  id: string;
  matrix_id: string;
  paper_id: string;
  column_id: string;
  value_text: string | null;
  value_number: number | null;
  value_bool: boolean | null;
  updated_at: number;
};

export type ResearchReport = {
  id: string;
  research_project_id: string;
  title: string;
  body_md: string;
  paper_ids: string[];
  claim_ids: string[];
  created_at: number;
  updated_at: number;
};

export async function researchProjectCreate(project: ResearchProject): Promise<ResearchProject> {
  return invoke("research_project_create", { project });
}
export async function researchProjectGet(id: string): Promise<ResearchProject> {
  return invoke("research_project_get", { id });
}
export async function researchProjectList(workspace_id: string | null = null, status: string | null = null, limit: number | null = null): Promise<ResearchProject[]> {
  return invoke("research_project_list", { workspaceId: workspace_id, status, limit });
}
export async function researchProjectUpdate(project: ResearchProject): Promise<ResearchProject> {
  return invoke("research_project_update", { project });
}
export async function researchProjectDelete(id: string): Promise<void> {
  return invoke("research_project_delete", { id });
}

export async function researchPaperAdd(paper: ResearchPaper): Promise<ResearchPaper> {
  return invoke("research_paper_add", { paper });
}
export async function researchPaperUpdate(paper: ResearchPaper): Promise<ResearchPaper> {
  return invoke("research_paper_update", { paper });
}
export async function researchPaperDelete(id: string): Promise<void> {
  return invoke("research_paper_delete", { id });
}
export async function researchPaperList(project_id: string): Promise<ResearchPaper[]> {
  return invoke("research_paper_list", { projectId: project_id });
}

export async function researchClaimAdd(claim: PaperClaim): Promise<PaperClaim> {
  return invoke("research_claim_add", { claim });
}
export async function researchClaimUpdate(claim: PaperClaim): Promise<PaperClaim> {
  return invoke("research_claim_update", { claim });
}
export async function researchClaimDelete(id: string): Promise<void> {
  return invoke("research_claim_delete", { id });
}
export async function researchClaimList(project_id: string, category: string | null = null, paper_id: string | null = null): Promise<PaperClaim[]> {
  return invoke("research_claim_list", { projectId: project_id, category, paperId: paper_id });
}

export async function researchMatrixCreate(matrix: ComparisonMatrix): Promise<ComparisonMatrix> {
  return invoke("research_matrix_create", { matrix });
}
export async function researchMatrixList(project_id: string): Promise<ComparisonMatrix[]> {
  return invoke("research_matrix_list", { projectId: project_id });
}
export async function researchMatrixDelete(id: string): Promise<void> {
  return invoke("research_matrix_delete", { id });
}
export async function researchMatrixColumns(matrix_id: string): Promise<MatrixColumn[]> {
  return invoke("research_matrix_columns", { matrixId: matrix_id });
}
export async function researchMatrixColumnAdd(column: MatrixColumn): Promise<MatrixColumn> {
  return invoke("research_matrix_column_add", { column });
}
export async function researchMatrixColumnDelete(id: string): Promise<void> {
  return invoke("research_matrix_column_delete", { id });
}
export async function researchMatrixCells(matrix_id: string): Promise<MatrixCell[]> {
  return invoke("research_matrix_cells", { matrixId: matrix_id });
}
export async function researchMatrixCellSet(cell: MatrixCell): Promise<MatrixCell> {
  return invoke("research_matrix_cell_set", { cell });
}

export async function researchReportSave(report: ResearchReport): Promise<ResearchReport> {
  return invoke("research_report_save", { report });
}
export async function researchReportGet(id: string): Promise<ResearchReport> {
  return invoke("research_report_get", { id });
}
export async function researchReportList(project_id: string): Promise<ResearchReport[]> {
  return invoke("research_report_list", { projectId: project_id });
}
export async function researchReportDelete(id: string): Promise<void> {
  return invoke("research_report_delete", { id });
}
export async function researchReportBuild(project_id: string): Promise<string> {
  return invoke("research_report_build", { projectId: project_id });
}
